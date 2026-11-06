(function (root) {
  "use strict";

  const BlobUI = root.BlobUI = root.BlobUI || {};
  let catalogPromise = null;
  BlobUI.DocsUI = Object.freeze({
    // Keep the loader lazy for the first documentation open and share concurrent requests.
    load() {
      if (!catalogPromise) {
        catalogPromise = BlobUI.DocsData.load().catch(error => {
          catalogPromise = null;
          throw error;
        });
      }
      return catalogPromise;
    },
  });
})(window);
