Feature: Account registration
  In order to start using the project
  As a user
  I want to create an account

  Scenario: Navigate to the registration form
    Given I am on the homepage
    When I click "Sign up"
    Then I should be on "/user/register"
    And I should see the heading "Sign up"
    And the response status code should be 200

  Scenario Outline: Register with invalid email
    Given I am on "/user/register"
    Then I should not see the form validation error "Please enter a valid email address."
    When I fill in "Email address" with "<email>"
    And I fill in "Password" with "<password>"
    And I press "Sign up"
    Then I should be on "/user/register"
    And I should see the heading "Sign up"
    And I should see the form validation error "Please enter a valid email address."

    Examples:
      | email                       | password |
      |                             |          |
      |                             | mypass   |
      | abc                         | mypass   |
      | abc@                        | mypass   |
      | a @x.cz                     | mypass   |
      | something@@somewhere.com    | mypass   |
      | email@[127.0.0.256]         | mypass   |
      | email@[::ffff:127.0.0.256]  | mypass   |
      | example@invalid-.com        | mypass   |
      | example@inv-.alid-.com      | mypass   |
      | trailingdot@shouldfail.com. | mypass   |

  Scenario: Register without providing a password
    Given I am on "/user/register"
    Then I should not see the form validation error "Please enter a password."
    When I fill in "Email address" with "test@example.com"
    And I press "Sign up"
    Then I should be on "/user/register"
    And I should see the heading "Sign up"
    And I should see the form validation error "Please enter a password."

  # A user might mistake the registration form for the login form. Transparently log in the user.
  Scenario: Log in by entering valid credentials in the registration form
    Given user "reuben-Tomas@demonic.demon.co.uk" with password "qwertyuiop"
    And I am on the user registration form
    When I fill in "Email address" with "reuben-Tomas@demonic.demon.co.uk"
    And I fill in "Password" with "qwertyuiop"
    And I press "Sign up"
    Then I should be on the homepage
    And I should see the link "Log out"
    But I should not see the link "Sign up"
    And I should not see the link "Log in"
