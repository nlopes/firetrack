use super::schema::categories;
use super::schema::categories::dsl;
use super::user::User;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::result::DatabaseErrorKind::{ForeignKeyViolation, UniqueViolation};
use diesel::result::Error::DatabaseError;
use serde::Serialize;
use std::fmt;

#[derive(Associations, Clone, Debug, PartialEq, Queryable, Serialize)]
#[belongs_to(User, foreign_key = "id")]
#[table_name = "categories"]
pub struct Category {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub user_id: i32,
    pub parent_id: Option<i32>,
}

// Possible errors thrown when handling categories.
#[derive(Debug, PartialEq)]
pub enum CategoryErrorKind {
    // The category with the given name and parent already exists.
    CategoryAlreadyExists {
        name: String,
        parent: Option<String>,
    },
    // A category could not be created due to a database error.
    CreationFailed(diesel::result::Error),
    // A category could not be deleted due to a database error.
    DeletionFailed(diesel::result::Error),
    // A category could not be deleted because it has child categories.
    HasChildren(i32),
    // Some required data is missing.
    MissingData(String),
    // A category could not be deleted because it does not exist.
    NotDeleted(i32),
    // A category was passed that belongs to the wrong user.
    ParentCategoryHasWrongUser(i32, i32),
}

impl fmt::Display for CategoryErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &*self {
            CategoryErrorKind::CategoryAlreadyExists { name, parent } => match parent {
                Some(p) => write!(
                    f,
                    "The child category '{}' already exists in the parent category '{}'",
                    name, p
                ),
                None => write!(f, "The root category '{}' already exists", name),
            },
            CategoryErrorKind::CreationFailed(ref err) => {
                write!(f, "Database error when creating category: {}", err)
            }
            CategoryErrorKind::DeletionFailed(ref err) => {
                write!(f, "Database error when deleting category: {}", err)
            }
            CategoryErrorKind::HasChildren(ref id) => write!(
                f,
                "The category with ID {} could not be deleted because it has child categories",
                id
            ),
            CategoryErrorKind::MissingData(ref err) => write!(f, "Missing data for field: {}", err),
            CategoryErrorKind::NotDeleted(ref id) => write!(
                f,
                "Could not delete category {} because it does not exist",
                id
            ),
            CategoryErrorKind::ParentCategoryHasWrongUser(ref expected_user_id, actual_user_id) => {
                write!(
                    f,
                    "Expected parent category for user {} instead of user {}",
                    expected_user_id, actual_user_id
                )
            }
        }
    }
}

/// Creates a category.
pub fn create(
    connection: &PgConnection,
    user: &User,
    name: &str,
    description: Option<&str>,
    parent: Option<&Category>,
) -> Result<Category, CategoryErrorKind> {
    // Validate the category name.
    let name = name.trim();
    if name.is_empty() {
        return Err(CategoryErrorKind::MissingData("category name".to_string()));
    }

    // Check that the parent category belongs to the same user.
    if let Some(parent) = parent {
        if parent.user_id != user.id {
            return Err(CategoryErrorKind::ParentCategoryHasWrongUser(
                user.id,
                parent.user_id,
            ));
        }
    }

    let parent_id = parent.map(|c| c.id);

    let result = diesel::insert_into(dsl::categories)
        .values((
            dsl::name.eq(&name),
            dsl::description.eq(description),
            dsl::user_id.eq(user.id),
            dsl::parent_id.eq(parent_id),
        ))
        .returning((
            dsl::id,
            dsl::name,
            dsl::description,
            dsl::user_id,
            dsl::parent_id,
        ))
        .get_result(connection);

    // Convert a UniqueViolation to a more informative CategoryAlreadyExists error.
    if let Err(DatabaseError(UniqueViolation, _)) = result {
        return Err(CategoryErrorKind::CategoryAlreadyExists {
            name: name.to_string(),
            parent: parent.map(|p| p.name.clone()),
        });
    }

    result.map_err(CategoryErrorKind::CreationFailed)
}

/// Retrieves the category with the given ID.
pub fn read(connection: &PgConnection, id: i32) -> Option<Category> {
    let category = dsl::categories.find(id).first::<Category>(connection);

    match category {
        Ok(c) => Some(c),
        Err(_) => None,
    }
}

/// Deletes the category with the given ID.
pub fn delete(connection: &PgConnection, id: i32) -> Result<(), CategoryErrorKind> {
    let result = diesel::delete(dsl::categories.filter(dsl::id.eq(id))).execute(connection);

    // Convert a ForeignKeyViolation to a more informative error.
    if let Err(DatabaseError(ForeignKeyViolation, _)) = result {
        return Err(CategoryErrorKind::HasChildren(id));
    }

    let result = result.map_err(CategoryErrorKind::DeletionFailed)?;

    // Throw an error if nothing was deleted.
    if result == 0 {
        return Err(CategoryErrorKind::NotDeleted(id));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_test::create_test_user;
    use crate::{establish_connection, get_database_url};
    use app::AppConfig;
    use diesel::result::Error;
    use std::collections::{BTreeMap, HashMap};

    // Tests creation of root level categories.
    #[test]
    fn test_create_root_category() {
        let conn = establish_connection(&get_database_url()).unwrap();
        let config = AppConfig::from_test_defaults();

        conn.test_transaction::<_, Error, _>(|| {
            // Create two test users that will serve as the owners of the test categories.
            let user1 = create_test_user(&conn, &config);
            let user2 = create_test_user(&conn, &config);

            // At the start of the test we should have no categories.
            assert_category_count(&conn, 0);

            // Create a root category without a description.
            let name1 = "Housing";
            let create_root_cat = || create(&conn, &user1, name1, None, None);
            let rootcat = create_root_cat().unwrap();
            assert_category(&rootcat, None, name1, None, user1.id, None);
            assert_category_count(&conn, 1);

            // We can create a root category for a different user with the same name.
            let rootcat_user2 = create(&conn, &user2, name1, None, None).unwrap();
            assert_category(&rootcat_user2, None, name1, None, user2.id, None);
            assert_category_count(&conn, 2);

            // We can create a root category with a description.
            let name2 = "Shopping";
            let desc = Some("Clothing, books, hobbies, …");
            let rootcat_desc = create(&conn, &user1, name2, desc, None).unwrap();
            assert_category(&rootcat_desc, None, name2, desc, user1.id, None);
            assert_category_count(&conn, 3);

            // Check that if we try to create a root category with a name that already exists we get
            // an error.
            assert_category_exists_err(create_root_cat().unwrap_err(), name1, None);

            Ok(())
        });
    }

    // Tests creation of child categories.
    #[test]
    fn test_create_child_categories() {
        let conn = establish_connection(&get_database_url()).unwrap();
        let config = AppConfig::from_test_defaults();

        // Test cases, keyed by category name, with optional description and parent category.
        let test_cases: BTreeMap<i8, (&str, Option<&str>, Option<i8>)> = [
            (0, ("Food", Some("Root category"), None)),
            (1, ("Groceries", None, Some(0))),
            (2, ("Groceries", Some("Same name as parent"), Some(1))),
            (3, ("Restaurants", Some("Eating out"), Some(0))),
            (4, ("Japanese restaurants", None, Some(3))),
            (5, ("Sushi", Some("Including delivery"), Some(4))),
            (6, ("Conveyor belt sushi", Some("Choo choo"), Some(5))),
        ]
        .iter()
        .cloned()
        .collect();

        conn.test_transaction::<_, Error, _>(|| {
            // Create two test users that will serve as the owners of the test categories.
            let user1 = create_test_user(&conn, &config);
            let user2 = create_test_user(&conn, &config);

            // At the start of the test we should have no categories.
            let mut count = 0;
            assert_category_count(&conn, count);

            let mut categories = HashMap::new();
            for (id, (name, description, parent_id)) in test_cases {
                let mut create_category = |u: &User| {
                    let parent = parent_id
                        .map(|id| categories.get(&(id, u.id)))
                        .unwrap_or(None);
                    // Create the category for test user 1.
                    let category = create(&conn, &u, name, description, parent);
                    categories.insert((id, u.id), category.unwrap());
                    count += 1;
                    assert_category_count(&conn, count);
                };

                // Different users should be able to create categories with the same names and the
                // same parent categories. Try creating each category for both test users.
                create_category(&user1);
                create_category(&user2);
            }

            // Check that if we try to create a category with a name that already exists for the
            // parent category we get an error. We are using test case 5 (Sushi) which has test case
            // 4 (Japanese restaurants) as parent category.
            let parent = categories.get(&(4, user1.id));
            assert_category_exists_err(
                create(&conn, &user1, "Sushi", None, parent).unwrap_err(),
                "Sushi",
                parent,
            );

            Ok(())
        });
    }

    // Test that an error is returned when creating a category with an empty name.
    #[test]
    fn test_create_with_empty_category_name() {
        let connection = establish_connection(&get_database_url()).unwrap();
        let config = AppConfig::from_test_defaults();

        connection.test_transaction::<_, Error, _>(|| {
            // Create a test user that will serve as the owner of the test categories.
            let user = create_test_user(&connection, &config);

            let mut empty_names = vec![
                "".to_string(),         // Empty string.
                " ".to_string(),        // Space.
                "\n".to_string(),       // Line feed.
                "\t".to_string(),       // Horizontal tab.
                '\u{0B}'.to_string(),   // Vertical tab.
                '\u{0C}'.to_string(),   // Form feed.
                '\u{85}'.to_string(),   // Next line.
                '\u{1680}'.to_string(), // Ogham space mark.
                '\u{2002}'.to_string(), // En space.
                '\u{2003}'.to_string(), // Em space.
                '\u{2004}'.to_string(), // Three-per-em space.
                '\u{2005}'.to_string(), // Four-per-em space.
                '\u{2006}'.to_string(), // Six-per-em space.
                '\u{2007}'.to_string(), // Figure space.
                '\u{2008}'.to_string(), // Punctuation space.
                '\u{2009}'.to_string(), // Thin space.
                '\u{200A}'.to_string(), // Hair space.
                '\u{2028}'.to_string(), // Line separator.
                '\u{2029}'.to_string(), // Paragraph separator.
                '\u{202F}'.to_string(), // Narrow no-break space.
                '\u{205F}'.to_string(), // Medium mathematical space.
                '\u{3000}'.to_string(), // Ideographic space.
            ];

            // Also test a combination of various whitespace characters.
            empty_names.push(format!(" \n\t{}{}{}", '\u{1680}', '\u{2005}', '\u{2028}'));

            for empty_name in empty_names {
                let created_category =
                    create(&connection, &user, &empty_name, None, None).unwrap_err();
                assert_eq!(
                    CategoryErrorKind::MissingData("category name".to_string()),
                    created_category
                );
            }

            Ok(())
        });
    }

    // Test that an error is returned when passing in a parent category from a different user.
    #[test]
    fn test_create_with_invalid_parent_category() {
        let connection = establish_connection(&get_database_url()).unwrap();
        let config = AppConfig::from_test_defaults();

        connection.test_transaction::<_, Error, _>(|| {
            // Create a test user that will serve as the owner of the test category.
            let user = create_test_user(&connection, &config);

            // Create a different user that owns some other category.
            let other_user = create_test_user(&connection, &config);

            // Try creating a new category that has a parent category belonging to a different user.
            // This should result in an error.
            let other_user_cat = create(&connection, &other_user, "Utilities", None, None).unwrap();
            let cat = create(
                &connection,
                &user,
                "Telecommunication",
                Some("Internet and telephone"),
                Some(&other_user_cat),
            )
            .unwrap_err();

            assert_eq!(
                CategoryErrorKind::ParentCategoryHasWrongUser(user.id, other_user.id),
                cat
            );

            Ok(())
        });
    }

    // Tests super::read().
    #[test]
    fn test_read() {
        let conn = establish_connection(&get_database_url()).unwrap();
        let config = AppConfig::from_test_defaults();

        conn.test_transaction::<_, Error, _>(|| {
            // When no category with the given ID exists, `None` should be returned.
            assert!(read(&conn, 1).is_none());

            // Create a root category and assert that the `read()` function returns it.
            let user = create_test_user(&conn, &config);
            let name = "Groceries";
            let result = create(&conn, &user, name, None, None).unwrap();
            let cat = read(&conn, result.id).unwrap();
            assert_category(&cat, Some(result.id), name, None, user.id, None);

            // Delete the category. Now the `read()` function should return `None` again.
            assert!(delete(&conn, cat.id).is_ok());
            assert!(read(&conn, cat.id).is_none());

            Ok(())
        });
    }

    // Tests super::delete().
    #[test]
    fn test_delete() {
        let conn = establish_connection(&get_database_url()).unwrap();
        let config = AppConfig::from_test_defaults();

        conn.test_transaction::<_, Error, _>(|| {
            // Initially there should not be any categories.
            assert_category_count(&conn, 0);

            // Create a root category. Now there should be one category.
            let user = create_test_user(&conn, &config);
            let name = "Healthcare";
            let cat = create(&conn, &user, name, None, None).unwrap();
            assert_category_count(&conn, 1);

            // Delete the category. This should not result in any errors, and there should again be
            // 0 categories in the database.
            assert!(delete(&conn, cat.id).is_ok());
            assert_category_count(&conn, 0);

            // Try deleting the category again.
            let result = delete(&conn, cat.id);
            assert!(result.is_err());
            assert_eq!(CategoryErrorKind::NotDeleted(cat.id), result.unwrap_err());

            Ok(())
        });
    }

    // Tests that a category which has a child category cannot be deleted.
    #[test]
    fn test_delete_with_child() {
        let conn = establish_connection(&get_database_url()).unwrap();
        let config = AppConfig::from_test_defaults();

        conn.test_transaction::<_, Error, _>(|| {
            // Create a root category.
            let user = create_test_user(&conn, &config);
            let name = "Lifestyle";
            let parent_cat = create(&conn, &user, name, None, None).unwrap();

            // Create a child category.
            let child_name = "Haircuts";
            create(&conn, &user, child_name, None, Some(&parent_cat)).unwrap();

            // Delete to delete the parent category. This should result in an error.
            let result = delete(&conn, parent_cat.id);
            assert!(result.is_err());
            assert_eq!(
                CategoryErrorKind::HasChildren(parent_cat.id),
                result.unwrap_err()
            );

            Ok(())
        });
    }

    // Checks that the given category matches the given values.
    fn assert_category(
        // The category to check.
        category: &Category,
        // The expected category ID. If None this will not be checked.
        id: Option<i32>,
        // The expected category name.
        name: &str,
        // The expected description.
        description: Option<&str>,
        // The expected user ID of the category owner.
        user_id: i32,
        // The expected parent category ID.
        parent_id: Option<i32>,
    ) {
        if let Some(id) = id {
            assert_eq!(id, category.id);
        }
        assert_eq!(name, category.name);
        assert_eq!(description.map(|d| d.to_string()), category.description);
        assert_eq!(user_id, category.user_id);
        assert_eq!(parent_id, category.parent_id);
    }

    // Checks that the number of categories stored in the database matches the expected count.
    fn assert_category_count(connection: &PgConnection, expected_count: i64) {
        let actual_count: i64 = dsl::categories
            .select(diesel::dsl::count_star())
            .first(connection)
            .unwrap();
        assert_eq!(expected_count, actual_count);
    }

    // Checks that the given error is an CategoryErrorKind::CategoryAlreadyExists error.
    fn assert_category_exists_err(error: CategoryErrorKind, name: &str, parent: Option<&Category>) {
        assert_eq!(
            error,
            CategoryErrorKind::CategoryAlreadyExists {
                name: name.to_string(),
                parent: parent.map(|p| p.name.clone())
            }
        );
    }
}