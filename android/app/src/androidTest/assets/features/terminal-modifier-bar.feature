@REQ_ANDR_011
Feature: Modifier Bar

  @REQ_ANDR_011
  Scenario: All modifier keys are displayed
    Given the app has launched
    Then the modifier bar shows ESC, TAB, CTRL, ALT, HOME, END, PGUP, PGDN keys

  @REQ_ANDR_011
  Scenario: Modifier keys can be toggled
    Given the app has launched
    When the CTRL key is tapped
    Then the CTRL key toggles appearance
    When the CTRL key is tapped again
    Then the CTRL key returns to default appearance
