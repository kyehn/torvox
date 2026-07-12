@REQ_SEARCH_001 @REQ_SEARCH_002
Feature: Text Search
  The terminal provides a search bar accessible from the session panel
  that highlights matching text and supports navigation.

  @REQ_SEARCH_001
  Scenario: Search bar opens from session panel button
    Given a terminal session is active
    When the user opens the search bar from the session panel
    Then the search bar is displayed at the bottom
    And the modifier bar is hidden

  @REQ_SEARCH_001
  Scenario: Search highlights matching text
    Given a terminal session is active with visible text
    When the user searches for "the"
    Then at least one match is highlighted on screen

  @REQ_SEARCH_001
  Scenario: Search previous and next navigate between matches
    Given the terminal has multiple "the" matches visible
    When the user presses "Next"
    Then the current match indicator changes
    When the user presses "Previous"
    Then the current match indicator returns

  @REQ_SEARCH_001
  Scenario: Case sensitive toggle filters results
    Given a terminal session is active with mixed case text
    When the user enables case-sensitive search
    And searches for "THE"
    Then only uppercase matches are highlighted

  @REQ_SEARCH_001
  Scenario: Search closes and clears highlights
    Given the terminal has search highlights active
    When the user closes the search bar
    Then all search highlights disappear
    And the modifier bar is visible again

  @REQ_SEARCH_002
  Scenario: Search auto-scrolls to off-screen match
    Given the terminal has scrolled content with "UNIQUE_MARKER"
    When the user searches for "UNIQUE_MARKER"
    And the match is not visible on the current screen
    Then the terminal scrolls to show the match

  @REQ_SEARCH_002
  Scenario: IME does not cover search bar
    Given the search bar is visible
    When the soft keyboard opens
    Then the search bar remains visible above the keyboard
