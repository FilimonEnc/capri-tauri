#!/usr/bin/env node
"use strict";

// Antigravity (AGY) hook -> Capri.
// Reads the Antigravity hook payload on stdin, translates it to Capri's event format,
// forwards it to Capri over HTTP, and prints the required JSON response on stdout.
// Rule #1: never get in Antigravity's way. Always output valid JSON and exit 0 fast.

const http = require("node:http");
const fs = require("node:fs");
const path = require("node:path");
const os = require("node:os");

const PORT = Number(process.env.CAPRI_PORT) || 47823;
const HOOK_TYPE = process.argv[2] || "PreToolUse";
const REPLY_MAX = 6000;

function respondAndExit() {
  if (HOOK_TYPE === "PreToolUse") {
    process.stdout.write(JSON.stringify({ decision: "allow" }) + "\n");
  } else if (HOOK_TYPE === "Stop") {
    process.stdout.write(JSON.stringify({ decision: "stop" }) + "\n");
  } else {
    process.stdout.write("{}\n");
  }
  process.exit(0);
}

setTimeout(respondAndExit, 2200).unref();

function cleanUserPrompt(text) {
  if (!text) return "";
  const match = String(text).match(/<USER_REQUEST>([\s\S]*?)<\/USER_REQUEST>/i);
  const raw = match ? match[1] : String(text);
  return raw.replace(/\s+/g, " ").trim().slice(0, 280);
}

function getLastAssistantMessage(transcriptPath) {
  if (!transcriptPath || !fs.existsSync(transcriptPath)) return "";
  try {
    const content = fs.readFileSync(transcriptPath, "utf8");
    const lines = content.trim().split("\n");
    for (let i = lines.length - 1; i >= 0; i--) {
      try {
        const step = JSON.parse(lines[i]);
        if (step.type === "PLANNER_RESPONSE" && step.content && String(step.content).trim()) {
          const reply = String(step.content).trim();
          return reply.length > REPLY_MAX ? reply.slice(0, REPLY_MAX) + "…" : reply;
        }
      } catch {
        // ignore malformed line
      }
    }
  } catch {
    // ignore read errors
  }
  return "";
}

function isOutsideWorkspace(filePath, workspacePaths) {
  if (!filePath || !Array.isArray(workspacePaths) || workspacePaths.length === 0) return false;
  try {
    const resolved = path.resolve(filePath).toLowerCase();
    return !workspacePaths.some((ws) => {
      const wsResolved = path.resolve(ws).toLowerCase();
      return resolved.startsWith(wsResolved);
    });
  } catch {
    return false;
  }
}

function isAutoExecutionOrTurbo(payload) {
  // 1. Explicit in hook payload (passed by Antigravity or test)
  if (payload) {
    if (payload.turbo === true || payload.isTurbo === true) return true;
    if (payload.turbo === false || payload.isTurbo === false) return false;
    const policy = payload.autoExecutionPolicy || payload.auto_execution_policy || payload.executionPolicy || payload.permissionMode || payload.permission_mode;
    if (typeof policy === "string") {
      if (/eager|turbo|always/i.test(policy) && !/strict|never|deny|ask/i.test(policy)) return true;
      if (/strict|ask|deny|review/i.test(policy)) return false;
    }
  }

  // 2. Explicit environment variables
  if (process.env.AGY_TURBO === "1" || process.env.AGY_TURBO === "true" || process.env.ANTIGRAVITY_TURBO === "1") {
    return true;
  }
  if (process.env.AGY_TURBO === "0" || process.env.AGY_TURBO === "false" || process.env.ANTIGRAVITY_TURBO === "0") {
    return false;
  }

  // 3. Capri's own config preference if configured
  try {
    const appData = process.env.APPDATA || (process.platform === "darwin" ? path.join(os.homedir(), "Library", "Application Support") : path.join(os.homedir(), ".config"));
    const capriCfgFiles = [
      path.join(appData, "capri-desktop", "config.json"),
      path.join(appData, "Capri", "config.json"),
    ];
    for (const f of capriCfgFiles) {
      if (fs.existsSync(f)) {
        const c = JSON.parse(fs.readFileSync(f, "utf8"));
        if (c.agyTurbo === "turbo" || c.agyMode === "turbo") return true;
        if (c.agyTurbo === "strict" || c.agyMode === "strict") return false;
      }
    }
  } catch {}

  const base = process.env.AGY_CONFIG_DIR || path.join(os.homedir(), ".gemini", "config");

  // 4. Project settings (~/.gemini/config/projects/*.json)
  const workspacePaths = payload?.workspacePaths || [];
  if (Array.isArray(workspacePaths) && workspacePaths.length > 0) {
    const projDir = path.join(base, "projects");
    if (fs.existsSync(projDir)) {
      try {
        const files = fs.readdirSync(projDir).filter((f) => f.endsWith(".json"));
        for (const file of files) {
          const content = fs.readFileSync(path.join(projDir, file), "utf8");
          const proj = JSON.parse(content);
          const resources = proj.projectResources?.resources || [];
          const matches = resources.some((r) => {
            const uri = r.gitFolder?.folderUri || r.folderUri || "";
            const cleanUri = decodeURIComponent(uri.replace(/^file:\/\/\/?/i, ""));
            return workspacePaths.some((ws) => {
              const wsNorm = path.resolve(ws).toLowerCase();
              const projNorm = path.resolve(cleanUri).toLowerCase();
              return wsNorm === projNorm || wsNorm.startsWith(projNorm) || projNorm.startsWith(wsNorm);
            });
          });
          if (matches && proj.settings) {
            const autoExec = proj.settings.autoExecutionPolicy;
            if (typeof autoExec === "string") {
              if (/eager|turbo|always/i.test(autoExec) && !/strict|ask|deny/i.test(autoExec)) return true;
              if (/strict|ask|deny|review/i.test(autoExec)) return false;
            }
            if (proj.settings.artifactReviewMode === "ARTIFACT_REVIEW_MODE_TURBO") return true;
          }
        }
      } catch {}
    }
  }

  // 5. Global userSettings in ~/.gemini/config/config.json
  const configFile = path.join(base, "config.json");
  if (fs.existsSync(configFile)) {
    try {
      const cfg = JSON.parse(fs.readFileSync(configFile, "utf8"));
      const userSettings = cfg.userSettings || {};
      const autoExec = userSettings.autoExecutionPolicy;
      if (typeof autoExec === "string") {
        if (/eager|turbo|always/i.test(autoExec) && !/strict|ask|deny/i.test(autoExec)) return true;
        if (/strict|ask|deny|review/i.test(autoExec)) return false;
      }
      if (userSettings.artifactReviewMode === "ARTIFACT_REVIEW_MODE_TURBO") return true;
      if (userSettings.browserJsExecutionPolicy === "BROWSER_JS_EXECUTION_POLICY_TURBO") return true;
    } catch {}
  }

  return false;
}

function isFileAccessAllowed(payload) {
  if (isAutoExecutionOrTurbo(payload)) return true;
  const base = process.env.AGY_CONFIG_DIR || path.join(os.homedir(), ".gemini", "config");
  const configFile = path.join(base, "config.json");
  if (fs.existsSync(configFile)) {
    try {
      const cfg = JSON.parse(fs.readFileSync(configFile, "utf8"));
      const policy = cfg.userSettings?.nonWorkspaceFileAccessPolicy;
      if (typeof policy === "string" && /allow/i.test(policy)) return true;
    } catch {}
  }
  return false;
}

function mapTool(toolCall) {
  const name = toolCall?.name || "";
  const args = toolCall?.args || {};

  switch (name) {
    case "read_file":
    case "view_file":
    case "view_file_outline":
    case "view_code_item":
      return {
        tool_name: "Read",
        tool_input: { file_path: args.Path || args.AbsolutePath || args.File || "" },
      };
    case "write_file":
    case "write_to_file":
      return {
        tool_name: "Write",
        tool_input: {
          file_path: args.Path || args.TargetFile || "",
          content_len: typeof args.Content === "string" ? args.Content.length : 0,
        },
      };
    case "edit_file":
    case "replace_file_content":
    case "multi_replace_file_content":
      return {
        tool_name: "Edit",
        tool_input: {
          file_path: args.Path || args.TargetFile || "",
          old_string_len: typeof args.Target === "string" ? args.Target.length : 0,
          new_string_len: typeof args.Replacement === "string" ? args.Replacement.length : 0,
        },
      };
    case "run_command":
      return {
        tool_name: "PowerShell",
        tool_input: {
          command: String(args.CommandLine || "").slice(0, 280),
          description: String(args.toolSummary || args.CommandLine || "").slice(0, 280),
        },
      };
    case "grep_search":
      return {
        tool_name: "Grep",
        tool_input: { pattern: String(args.Query || "").slice(0, 280) },
      };
    case "find_by_name":
    case "list_dir":
      return {
        tool_name: "Glob",
        tool_input: { pattern: String(args.Pattern || args.DirectoryPath || "*").slice(0, 280) },
      };
    case "search_web":
      return {
        tool_name: "WebSearch",
        tool_input: { query: String(args.query || "").slice(0, 280) },
      };
    case "read_url_content":
      return {
        tool_name: "WebFetch",
        tool_input: { url: String(args.Url || "").slice(0, 280) },
      };
    case "invoke_subagent":
      return {
        tool_name: "Agent",
        tool_input: {
          description: String(args.Role || args.TypeName || "subagent").slice(0, 280),
          subagent_type: String(args.TypeName || "agent").slice(0, 100),
        },
      };
    case "ask_question":
      return {
        tool_name: "AskUserQuestion",
        tool_input: {
          question: String(args.questions?.[0]?.question || "").slice(0, 280),
        },
      };
    default:
      return {
        tool_name: name || "Tool",
        tool_input: {},
      };
  }
}

function buildEvents(payload) {
  const base = {
    // marked as Antigravity's, so she only follows it when it's the assistant she watches
    session_id: `agy:${payload.conversationId || "session"}`,
    cwd: (Array.isArray(payload.workspacePaths) && payload.workspacePaths[0]) || process.cwd(),
  };

  if (HOOK_TYPE === "SessionStart") {
    return [{ delay: 0, evt: { ...base, hook_event_name: "SessionStart", source: "startup" } }];
  }

  if (HOOK_TYPE === "PreInvocation") {
    if (payload.invocationNum && payload.invocationNum > 1) return [];
    const prompt = cleanUserPrompt(payload.lastUserInput);
    if (!prompt) return [];
    return [{ delay: 0, evt: { ...base, hook_event_name: "UserPromptSubmit", prompt } }];
  }

  if (HOOK_TYPE === "PreToolUse") {
    const mapped = mapTool(payload.toolCall);
    const events = [{ delay: 0, evt: { ...base, hook_event_name: "PreToolUse", ...mapped } }];
    const toolName = payload.toolCall?.name || "";
    const filePath = mapped.tool_input?.file_path || "";
    const turbo = isAutoExecutionOrTurbo(payload);

    if (toolName === "run_command") {
      // In turbo / eager auto-execution mode, commands execute automatically.
      // Capri should not signal that user permission is needed.
      if (!turbo) {
        const cmd = mapped.tool_input.command || mapped.tool_input.description || "PowerShell";
        events.push({
          delay: 350,
          evt: {
            ...base,
            hook_event_name: "Notification",
            notification_type: "permission_prompt",
            title: "Permission needed",
            message: cmd,
          },
        });
      }
    } else if (toolName === "ask_question") {
      events.push({
        delay: 250,
        evt: {
          ...base,
          hook_event_name: "Notification",
          notification_type: "elicitation_dialog",
          title: "Question",
          message: mapped.tool_input.question || "Antigravity has a question for you",
        },
      });
    } else if (filePath && isOutsideWorkspace(filePath, payload.workspacePaths)) {
      if (!isFileAccessAllowed(payload)) {
        events.push({
          delay: 350,
          evt: {
            ...base,
            hook_event_name: "Notification",
            notification_type: "permission_prompt",
            title: "Permission needed",
            message: filePath,
          },
        });
      }
    } else if (toolName === "invoke_subagent") {
      events.push({
        delay: 200,
        evt: {
          ...base,
          hook_event_name: "SubagentStart",
          agent_type: mapped.tool_input.subagent_type,
        },
      });
    }
    return events;
  }

  if (HOOK_TYPE === "PostToolUse") {
    const mapped = mapTool(payload.toolCall);
    const events = [];
    if (payload.error) {
      events.push({
        delay: 0,
        evt: {
          ...base,
          hook_event_name: "PostToolUseFailure",
          ...mapped,
          error: String(payload.error).slice(0, 280),
        },
      });
    } else {
      events.push({
        delay: 0,
        evt: {
          ...base,
          hook_event_name: "PostToolUse",
          ...mapped,
        },
      });
    }
    if (payload.toolCall?.name === "invoke_subagent") {
      events.push({
        delay: 100,
        evt: {
          ...base,
          hook_event_name: "SubagentStop",
          agent_type: mapped.tool_input.subagent_type,
        },
      });
    }
    return events;
  }

  if (HOOK_TYPE === "Stop") {
    if (payload.terminationReason === "ERROR" || payload.error) {
      return [
        {
          delay: 0,
          evt: {
            ...base,
            hook_event_name: "StopFailure",
            error: String(payload.error || payload.terminationReason || "Error").slice(0, 280),
          },
        },
      ];
    }
    const last_assistant_message = getLastAssistantMessage(payload.transcriptPath);
    return [{ delay: 0, evt: { ...base, hook_event_name: "Stop", last_assistant_message } }];
  }

  return [];
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function postEvent(evt) {
  return new Promise((resolve) => {
    const body = JSON.stringify(evt);
    const req = http.request(
      {
        host: "127.0.0.1",
        port: PORT,
        path: "/event",
        method: "POST",
        headers: {
          "content-type": "application/json",
          "content-length": Buffer.byteLength(body),
        },
        timeout: 600,
      },
      (res) => {
        res.resume();
        res.on("end", resolve);
      }
    );
    req.on("timeout", () => {
      req.destroy();
      resolve();
    });
    req.on("error", () => resolve());
    req.end(body);
  });
}

let raw = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", (chunk) => {
  raw += chunk;
  if (raw.length > 8 * 1024 * 1024) respondAndExit();
});
process.stdin.on("error", respondAndExit);
process.stdin.on("end", async () => {
  let payload = {};
  try {
    if (raw.trim()) payload = JSON.parse(raw);
  } catch {
    return respondAndExit();
  }

  const items = buildEvents(payload);
  for (const { delay, evt } of items) {
    if (delay > 0) await sleep(delay);
    await postEvent(evt);
  }
  respondAndExit();
});
