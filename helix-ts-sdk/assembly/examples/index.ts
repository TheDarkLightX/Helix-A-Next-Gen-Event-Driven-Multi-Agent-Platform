// This file can be used to select which example agent to build.
// You would typically have different build configurations in asconfig.json
// or use a script to compile each example into its own .wasm file.

// To build a specific example, you can comment out the others and ensure
// only one `agent_main` is exported, or your build script/config
// should point to the specific example file (e.g., state-example.ts) as the entry.

// Example: Exporting state-example's main function
export { agent_main as state_example_main } from "./state-example";

// Example: Exporting credential-example's main function
export { agent_main as credential_example_main } from "./credential-example";

// Example: Exporting random-example's main function
export { agent_main as random_example_main } from "./random-example";

// Example: Exporting http-example's main function
export { agent_main as http_example_main } from "./http-example";

// Example: Exporting daily-briefing-agent's main function
export { agent_main as daily_briefing_agent_main } from "./daily-briefing-agent";

// Default main for a combined/test build (less common for separate examples)
// For individual example builds, the asconfig.json entry file would be
// the specific example.ts file.
// If you wanted a "default" or "test all" (though not practical for agent_main),
// you might do something like this, but it's not standard for agent entry points.
/*
import * as StateExample from "./state-example";
import * as CredentialExample from "./credential-example";
import * as RandomExample from "./random-example";
import * as HttpExample from "./http-example";

export function agent_main(): void {
    log("Running all examples (conceptual - not a typical agent_main pattern)");
    StateExample.agent_main();
    CredentialExample.agent_main();
    RandomExample.agent_main();
    HttpExample.agent_main();
}
*/
// For now, just exporting them with distinct names.
// The build process will determine which `agent_main` (or renamed export) is used as the entry.