// Chrome has no `browser` namespace (Firefox has both). The rest of the add-on is written
// against `browser`, so Chrome gets one here. REYNARD_CHROME tells the code which one it is.
// In Firefox this file changes nothing.
var REYNARD_CHROME = typeof browser === "undefined";
if (REYNARD_CHROME) globalThis.browser = chrome;
