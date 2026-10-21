(function (global) {
  "use strict";

  const blobUi = global.BlobUI || (global.BlobUI = {});

  function copy(value) {
    if (value === undefined) return undefined;
    return JSON.parse(JSON.stringify(value));
  }

  function createEditSession(options) {
    if (!options || typeof options.read !== "function" || typeof options.restore !== "function" || typeof options.equals !== "function") {
      throw new TypeError("An edit session requires read, restore, and equals functions.");
    }

    let baseline = copy(options.read());

    function commit(patch) {
      if (patch === undefined) {
        baseline = copy(options.read());
      } else {
        baseline = Object.assign({}, baseline, copy(patch));
      }
      return copy(baseline);
    }

    function discard() {
      const saved = copy(baseline);
      options.restore(saved);
      return copy(saved);
    }

    return Object.freeze({
      commit,
      discard,
      getBaseline: () => copy(baseline),
      isDirty: () => !options.equals(options.read(), baseline),
    });
  }

  blobUi.createEditSession = createEditSession;
})(window);
