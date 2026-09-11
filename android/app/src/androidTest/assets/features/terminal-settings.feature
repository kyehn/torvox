@REQ_ANDR_004 @REQ_ANDR_008
Feature: Settings — General

  @REQ_ANDR_004
  Scenario: Settings screen displays all sections
    Given the app has launched
    When the user opens the settings screen
    Then theme selector and font size slider are displayed

  @REQ_ANDR_008
  Scenario: Font size can be adjusted
    Given the user is on the settings screen
    Then the font size slider exists
    When the slider is adjusted
    Then the terminal font size changes
