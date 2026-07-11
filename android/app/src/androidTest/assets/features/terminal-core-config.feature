@wip
@REQ_CORE_005 @REQ_CORE_006
Feature: Terminal Core Configuration
  The terminal configuration supports serialization, deserialization,
  and persistence across app lifecycle events.

  @REQ_CORE_005
  Scenario: Default configuration has system shell
    Given a default terminal configuration
    Then the shell is set to SystemDefault
    And the rows are 24
    And the columns are 80

  @REQ_CORE_005
  Scenario: Custom shell configuration is preserved
    Given a terminal configuration with custom shell "/bin/zsh"
    Then the shell is Custom("/bin/zsh")

  @REQ_CORE_005
  Scenario: Configuration dimensions are configurable
    Given a terminal configuration with 30 rows and 100 columns
    Then the rows are 30
    And the columns are 100

  @REQ_CORE_006
  Scenario: Configuration serializes to JSON
    Given a default terminal configuration
    When the configuration is serialized to JSON
    Then the JSON contains "rows" and "cols" fields
    And the JSON can be deserialized back to a matching configuration

  @REQ_CORE_006
  Scenario: Configuration survives roundtrip through JSON
    Given a terminal configuration with custom shell and 40x120 dimensions
    When the configuration is serialized and deserialized
    Then the restored configuration matches the original

  @REQ_CORE_006
  Scenario: Theme configuration serializes color values
    Given a theme configuration with Catppuccin Mocha colors
    When the theme is serialized to JSON
    Then the JSON contains foreground and background color values
