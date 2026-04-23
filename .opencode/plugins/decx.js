import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const BOOTSTRAP_MARKER = "DECX_OPENCODE_BOOTSTRAP";

function stripFrontmatter(content) {
  const match = content.match(/^---\n[\s\S]*?\n---\n([\s\S]*)$/);
  return match ? match[1].trim() : content.trim();
}

function readUsingDecx(skillsDir) {
  const skillPath = path.join(skillsDir, "using-decx", "SKILL.md");
  if (!fs.existsSync(skillPath)) return null;
  return stripFrontmatter(fs.readFileSync(skillPath, "utf8"));
}

function buildBootstrap(skillsDir) {
  const usingDecx = readUsingDecx(skillsDir);
  if (!usingDecx) return null;

  return [
    `<${BOOTSTRAP_MARKER}>`,
    "You have DECX skills installed.",
    "",
    "IMPORTANT: The `using-decx` skill content is included below. Treat it as already loaded; do not load `using-decx` again unless you need to re-read the source.",
    "",
    usingDecx,
    "",
    "OpenCode tool mapping:",
    "- Skill tool -> OpenCode's native `skill` tool",
    "- Subagents -> OpenCode agent mention syntax when available",
    "- Shell commands -> OpenCode native shell command tools",
    "- File operations -> OpenCode native file tools",
    `</${BOOTSTRAP_MARKER}>`,
  ].join("\n");
}

function injectBootstrap(output, bootstrap) {
  if (!bootstrap || !Array.isArray(output?.messages)) return;

  const firstUser = output.messages.find((message) => (message?.info?.role ?? message?.role) === "user");
  if (!firstUser || !Array.isArray(firstUser.parts)) return;

  if (firstUser.parts.some((part) => part?.type === "text" && typeof part.text === "string" && part.text.includes(BOOTSTRAP_MARKER))) {
    return;
  }

  const ref = firstUser.parts[0] ?? {};
  firstUser.parts.unshift({
    ...ref,
    type: "text",
    text: bootstrap,
  });
}

export const DecxPlugin = async () => {
  const skillsDir = path.resolve(__dirname, "../../skills");

  return {
    config: async (config) => {
      config.skills ??= {};
      config.skills.paths ??= [];
      if (!config.skills.paths.includes(skillsDir)) {
        config.skills.paths.push(skillsDir);
      }
    },
    "experimental.chat.messages.transform": async (_input, output) => {
      injectBootstrap(output, buildBootstrap(skillsDir));
    },
  };
};
