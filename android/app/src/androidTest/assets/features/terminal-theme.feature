@REQ_ANDR_004
Feature: Theme Selection

  @REQ_ANDR_004
  Scenario: Theme switching works
    Given the user is on the settings screen
    When the user selects a different theme
    Then the terminal theme updates

  @REQ_ANDR_004
  Scenario: Default theme is applied on first launch
    Given the app has launched
    Then the default theme is applied to the terminal
