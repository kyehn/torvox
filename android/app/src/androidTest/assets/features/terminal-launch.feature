@REQ_TERM_001 @REQ_ANDR_004 @REQ_SYS_002
Feature: Terminal Launch

  @REQ_TERM_001
  Scenario: Terminal screen renders on launch
    Given the app has launched
    Then the terminal screen is displayed
    And the modifier bar is visible
    And the terminal content area has positive dimensions

  @REQ_ANDR_004
  Scenario: SurfaceView renders above Compose UI
    Given the app has launched
    Then the SurfaceView is visible
    And it renders above the Compose layout
