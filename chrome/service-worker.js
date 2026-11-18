// Chrome runs the background as a service worker, which is a single file. This loads the same
// scripts that the Firefox background page lists in its manifest.
importScripts("compat.js", "config.js", "background.js");
