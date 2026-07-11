@REQ_ANDR_008 @REQ_ANDR_015
Feature: Font Management

  @REQ_ANDR_008
  Scenario: Font switching works with valid font
    Given the app has launched
    When the user opens settings
    And changes the font family
    Then the terminal font updates without error

  @REQ_ANDR_015
  Scenario: Invalid font does not crash the app
    Given the app has launched
    When the user attempts to load an invalid font file
    Then the app does not crash
    And the previous working font is preserved
    And a user-visible error message is shown

  @REQ_ANDR_015
  Scenario: Font files are stored in private directory
    Given the app has launched
    Then font files are stored in the application's private fonts directory
