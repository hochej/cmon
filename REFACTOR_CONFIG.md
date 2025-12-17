 Plan: Refactor Config to Idiomatic Rust Serde Defaults

 Scope


 Problem

 The config system in src/models/config.rs has significant redundancy:
 1. 8 boilerplate default_* functions that exist only to satisfy serde's field-level #[serde(default = "fn")]
 2. Defaults defined TWICE - once in the function AND again in manual impl Default blocks
 3. Mixed approaches within structs - some fields use #[serde(default)], others use #[serde(default = "fn")]

 Solution

 Use the idiomatic Rust pattern: struct-level #[serde(default)] with a single impl Default as the source of truth.

 File to Modify

 - /Users/hochej/dev/cmon/src/models/config.rs

 ---
 Implementation Steps

 Step 1: Refactor RefreshConfig [DONE]

 Before: Field-level #[serde(default = "default_jobs_interval")] etc.
 After: Add #[serde(default)] at struct level, remove all field-level attributes, keep existing impl Default

 Step 2: Refactor DisplayConfig [DONE]

 - Add #[serde(default)] at struct level
 - Remove all field-level #[serde(default)] and #[serde(default = "fn")] attributes
 - Fix bug: change default_job_name_max_length() to literal 35 in impl Default

 Step 3: Refactor BehaviorConfig [DONE]

 - Add #[serde(default)] at struct level
 - Remove #[serde(default = "default_true")] from both fields
 - Keep existing impl Default

 Step 4: Refactor SystemConfig [DONE]

 - Keep #[derive(Default)] (clippy: derivable_impls - all fields use type-default)
 - Add #[serde(default)] at struct level
 - Remove field-level #[serde(default)]

 Step 5: Refactor TuiConfig [DONE]

 - Keep #[derive(Default)] (clippy: derivable_impls - all fields impl Default)
 - Add #[serde(default)] at struct level
 - Remove all field-level #[serde(default)]

 Step 6: Delete All default_* Functions

 Remove these 8 functions entirely:
 - default_jobs_interval()
 - default_nodes_interval()
 - default_fairshare_interval()
 - default_idle_threshold()
 - default_true()
 - default_view()
 - default_theme()
 - default_job_name_max_length()

 ---
 Target Pattern (Example)

 #[derive(Debug, Clone, Deserialize, Serialize)]
 #[serde(default)]  // struct-level
 pub struct ExampleConfig {
     pub field: String,  // no field-level serde attributes
 }

 impl Default for ExampleConfig {
     fn default() -> Self {
         Self {
             field: "value".to_string(),  // single source of truth
         }
     }
 }
