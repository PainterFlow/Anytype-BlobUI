(function (root) {
  "use strict";
  const UI = root.BlobUI;
  const C = UI.Components;
  const featureLabels = ["Custom Accent Color", "Custom Font", "Custom Space Banner", "Animated Text Effects", "UI fixes",
    "Calendar Colors", "Read and use your data for BlobUI features", "Corrupt/Modify your Data", "Sell/Collect your Data"];
  const features = useApi => featureLabels.map((label, index) => ({ label, included: index < (useApi ? 7 : 5) }));
  UI.ScreenDefinitions = Object.assign(UI.ScreenDefinitions || {}, {
    "onboarding-welcome": content => C.Button({
      attributes: {
        id: "onboarding-welcome",
        class: "onboarding-welcome",
        "aria-labelledby": "onboarding-title",
        "aria-describedby": "onboarding-hint"
      },
      content
    }),
    "onboarding-no-api-option": () => C.OnboardingChoice({
      attributes: {
        id: "onboarding-no-api-option",
        class: "onboarding-option",
        "aria-pressed": "false",
        "data-api": "false"
      },
      title: "No API Key",
      features: features(false)
    }),
    "onboarding-api-option": () => C.OnboardingChoice({
      attributes: {
        id: "onboarding-api-option",
        class: "onboarding-option recommended-option",
        "aria-pressed": "false",
        "data-api": "true"
      },
      title: "Use API Key",
      recommendedLabel: "Recommended",
      features: features(true)
    }),
    "connection-form-fields": content => C.Group({
      attributes: {
        class: "font-fields"
      },
      content
    }),
    "connection-key": content => C.TextField({
      inputAttributes: {
        id: "connection-key",
        type: "password",
        autocomplete: "off",
        spellcheck: "false",
        maxlength: "4096",
        placeholder: "Paste your API v2 key",
        "aria-describedby": "connection-error",
        required: ""
      },
      visibility: {
        buttonAttributes: {
          id: "connection-key-visibility",
          class: "visibility-toggle",
          "aria-controls": "connection-key",
          "aria-pressed": "false",
          "aria-label": "Show API key",
          title: "Show API key"
        }
      },
      labelContent: content
    }),
    "connection-port": content => C.TextField({
      inputAttributes: {
        id: "connection-port",
        type: "text",
        inputmode: "numeric",
        autocomplete: "off",
        maxlength: "5",
        placeholder: "31009",
        "aria-describedby": "connection-port-hint connection-error"
      },
      visibility: {
        buttonAttributes: {
          id: "connection-port-visibility",
          class: "visibility-toggle",
          "aria-controls": "connection-port",
          "aria-pressed": "true",
          "aria-label": "Hide port",
          title: "Hide port"
        }
      },
      labelContent: content
    }),
    "connection-back": content => C.Button({
      attributes: {
        id: "connection-back",
        class: "connection-back"
      },
      content
    }),
    "connection-confirm": content => C.Button({
      attributes: {
        id: "connection-confirm",
        class: "connection-confirm",
        type: "submit"
      },
      content
    }),
  });
})(window);
