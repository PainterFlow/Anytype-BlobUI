import React, { useLayoutEffect, useRef, useSyncExternalStore } from "react";
import { CodexProjects } from "./codex-projects.jsx";
import { ObjectMentions } from "./object-mentions.jsx";
import { CodexIntegration } from "./codex-integration.jsx";
import { CodexPermissions } from "./codex-permissions.jsx";
import { Tabs } from "../shared/tabs.jsx";

export function CodexPage() {
  const root = useRef(null);
  const controller=window.BlobUI.CodexPage;
  const selected=useSyncExternalStore(controller.subscribeTabs,controller.getTabSnapshot,controller.getTabSnapshot);
  useLayoutEffect(() => {
    window.BlobUI.CodexPage.attach({ root: root.current });
  }, []);
  return <div className="codex-react-page" ref={root}>
    <Tabs id="codex-tabs" label="Anyblob" selected={selected} onSelect={index=>controller.select(index)} useChildPanels items={[
      {tabId:"codex-tab-0",panelId:"codex-panel-0",title:"Settings"},
      {tabId:"codex-tab-1",panelId:"codex-panel-1",title:"Permissions"},
      {tabId:"codex-tab-2",panelId:"codex-panel-2",title:"Projects"},
    ]}>
      {[
        <section><CodexIntegration /><ObjectMentions /></section>,
        <section><CodexPermissions /></section>,
        <section><CodexProjects /></section>,
      ]}
    </Tabs>
  </div>;
}
