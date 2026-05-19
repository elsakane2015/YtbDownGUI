// Typed wrappers around Tauri's invoke + event listen.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type ToolVersion = { name: string; version: string };

export type AccountStatus = {
  site_id: string;
  display_name: string;
  logged_in: boolean;
  cookie_count: number;
};

export const probeToolVersions = () =>
  invoke<ToolVersion[]>("probe_tool_versions");

export const listAccounts = () => invoke<AccountStatus[]>("list_accounts");

export const startLogin = (siteId: string) =>
  invoke<void>("start_login", { siteId });

export const finishLogin = (siteId: string) =>
  invoke<number>("finish_login", { siteId });

export const cancelLogin = () => invoke<void>("cancel_login");

export const logout = (siteId: string) => invoke<void>("logout", { siteId });

export const exportCookiesNetscape = (siteId: string) =>
  invoke<string>("export_cookies_netscape", { siteId });

export const onAccountUpdated = (
  cb: (siteId: string) => void,
): Promise<UnlistenFn> => listen<string>("account:updated", (e) => cb(e.payload));

export const onLoginEvent = (
  kind: "succeeded" | "cancelled" | "timeout" | "failed",
  cb: (payload: string) => void,
): Promise<UnlistenFn> =>
  listen<string>(`login:${kind}`, (e) => cb(e.payload));
