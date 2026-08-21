import { describe, expect, it } from "vitest";

import {
  AGENT_TEAMS_REPORTING_CONTRACT,
  BRIDGE_DEFAULT_CLAUDE_MD,
} from "./bridge-default-claude-md";

describe("Agent Teams reporting contract", () => {
  it("is conditional and does not force a team for every task", () => {
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain(
      "If you choose to use Agent Teams",
    );
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain(
      "Agent Teams are optional",
    );
    expect(AGENT_TEAMS_REPORTING_CONTRACT).not.toMatch(
      /always use Agent Teams|for every task/i,
    );
  });

  it("requires bounded assignments and a finite expected teammate set", () => {
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain(
      "finite, named teammate set",
    );
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain(
      "bounded independent task",
    );
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain(
      "explicit deliverable and stop condition",
    );
  });

  it("requires delivery through SendMessage before idle", () => {
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain(
      "native `SendMessage` tool",
    );
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain(
      '`recipient: "team-lead"`',
    );
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain("before becoming idle");
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain(
      "status, findings, artifact paths, and blockers",
    );
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain("not the report itself");
  });

  it("permits one bounded recovery without replacement or an infinite loop", () => {
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain(
      "at most one recovery `SendMessage`",
    );
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain("that same teammate");
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain(
      "Do not create a replacement teammate",
    );
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain(
      "retry in a loop, or wait indefinitely",
    );
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain(
      "synthesize only the reports that were actually delivered",
    );
  });

  it("installs the reporting contract in the shared Bridge default", () => {
    expect(
      BRIDGE_DEFAULT_CLAUDE_MD.split("## Agent Teams delivery contract"),
    ).toHaveLength(2);
  });
});
