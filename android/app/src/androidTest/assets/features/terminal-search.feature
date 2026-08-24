@REQ_SEARCH_001 @REQ_SEARCH_002
Feature: Text Search
  The terminal provides a search bar accessible from the session panel
  that highlights matching text and supports navigation.

  NOTE: search is wired end-to-end (native `searchAllInScrollback` path —
  the old "getTerminalText stub" note is long obsolete: Bridge methods all
  delegate to real JNI via NativeQueryPort). The @wip scenarios below will
  be re-enabled one by one in a cucumber round; their step definitions may
  need updating for the system-ActionMode menu / current UI nodes.

  @REQ_SEARCH_001
  @wip
  Scenario: Search bar opens from session panel button
    Given a terminal session is active
    When the user opens the search bar from the session panel
    Then the search bar is displayed at the bottom
    And the modifier bar is hidden

  @REQ_SEARCH_001
  @wip
  Scenario: Search closes and clears highlights
    Given the terminal has search highlights active
    When the user closes the search bar
    Then all search highlights disappear
    And the modifier bar is visible again

  @REQ_SEARCH_002
  @wip
  Scenario: IME does not cover search bar
    Given the search bar is visible
    When the soft keyboard opens
    Then the search bar remains visible above the keyboard
