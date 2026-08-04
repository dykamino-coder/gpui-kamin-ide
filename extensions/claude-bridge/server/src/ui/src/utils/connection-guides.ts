// Connection guide data for all supported plugins.
// Single source of truth — used by ConnectionGuide component on both /tokens and plugin modal.

export interface GuideBlock {
  label: string
  code: string       // displayed in UI (can be multiline)
  copyCode?: string  // copied to clipboard (one-liner); falls back to code if absent
}

export interface GuideSection {
  heading?: string
  text?: string
  blocks: GuideBlock[]
}

export interface Guide {
  id: string
  label: string
  sections: GuideSection[]
  hint?: string
}

function getBase(): string {
  return `${location.protocol}//${location.host}`
}

export function getConnectionGuides(): Guide[] {
  const base = getBase()

  return [
    {
      id: 'claude-code',
      label: 'Claude Code',
      hint: 'Replace <your-token> with a token from the table above. After setup, just run: claude',
      sections: [
        {
          heading: 'First-time setup (run ONCE on each new machine)',
          text: 'Merges into existing config — won\'t overwrite your settings.',
          blocks: [
            {
              label: 'Bash / Linux / macOS',
              copyCode: `mkdir -p ~/.claude && node -e 'const fs=require("fs"),h=require("os").homedir();function merge(p,d){let j={};if(fs.existsSync(p)){try{let r=fs.readFileSync(p);j=JSON.parse((r[0]===0xFF&&r[1]===0xFE?r.slice(2).toString("utf16le"):r.toString("utf8")).replace(/^\\uFEFF/,""))}catch(e){console.warn(e.message)}}const e=d.env;delete d.env;Object.assign(j,d);if(e)j.env=Object.assign(j.env||{},e);fs.writeFileSync(p,JSON.stringify(j,null,2),"utf8")}merge(h+"/.claude/settings.json",{apiKeyHelper:"echo <your-token>",skipDangerousModePermissionPrompt:true,skipWebFetchPreflight:true,env:{ANTHROPIC_BASE_URL:"${base}"}});merge(h+"/.claude.json",{hasCompletedOnboarding:true});try{fs.unlinkSync(h+"/.claude/.credentials.json")}catch{}' && claude`,
              code: `mkdir -p ~/.claude

node -e '
  const fs = require("fs");
  const h = require("os").homedir();

  function merge(filePath, data) {
    let json = {};
    if (fs.existsSync(filePath)) {
      try {
        let raw = fs.readFileSync(filePath);
        const text = raw[0] === 0xFF && raw[1] === 0xFE
          ? raw.slice(2).toString("utf16le")
          : raw.toString("utf8").replace(/^\\uFEFF/, "");
        json = JSON.parse(text);
      } catch (e) { console.warn(e.message); }
    }
    const env = data.env; delete data.env;
    Object.assign(json, data);
    if (env) json.env = Object.assign(json.env || {}, env);
    fs.writeFileSync(filePath, JSON.stringify(json, null, 2), "utf8");
  }

  merge(h + "/.claude/settings.json", {
    apiKeyHelper: "echo <your-token>",
    skipDangerousModePermissionPrompt: true,
    skipWebFetchPreflight: true,
    env: { ANTHROPIC_BASE_URL: "${base}" }
  });
  merge(h + "/.claude.json", { hasCompletedOnboarding: true });
  try { fs.unlinkSync(h + "/.claude/.credentials.json"); } catch {}
'

claude`,
            },
            {
              label: 'CMD (Windows)',
              copyCode: `mkdir "%USERPROFILE%\\.claude" 2>nul & node -e "const fs=require('fs'),h=process.env.USERPROFILE;function merge(p,d){let j={};if(fs.existsSync(p)){try{let r=fs.readFileSync(p);j=JSON.parse((r[0]===0xFF&&r[1]===0xFE?r.slice(2).toString('utf16le'):r.toString('utf8')).replace(/^\\uFEFF/,''))}catch(e){console.warn(e.message)}}const e=d.env;delete d.env;Object.assign(j,d);if(e)j.env=Object.assign(j.env||{},e);fs.writeFileSync(p,JSON.stringify(j,null,2),'utf8')}merge(h+'/.claude/settings.json',{apiKeyHelper:'echo <your-token>',skipDangerousModePermissionPrompt:true,skipWebFetchPreflight:true,env:{ANTHROPIC_BASE_URL:'${base}'}});merge(h+'/.claude.json',{hasCompletedOnboarding:true});try{fs.unlinkSync(h+'/.claude/.credentials.json')}catch{}" && claude`,
              code: `mkdir "%USERPROFILE%\\.claude" 2>nul

node -e "
  const fs = require('fs');
  const h = process.env.USERPROFILE;

  function merge(filePath, data) {
    let json = {};
    if (fs.existsSync(filePath)) {
      try {
        let raw = fs.readFileSync(filePath);
        const text = raw[0]===0xFF && raw[1]===0xFE
          ? raw.slice(2).toString('utf16le')
          : raw.toString('utf8').replace(/^\\uFEFF/,'');
        json = JSON.parse(text);
      } catch(e) { console.warn(e.message); }
    }
    const e = data.env; delete data.env;
    Object.assign(json, data);
    if (e) json.env = Object.assign(json.env || {}, e);
    fs.writeFileSync(filePath, JSON.stringify(json, null, 2), 'utf8');
  }

  merge(h + '/.claude/settings.json', {
    apiKeyHelper: 'echo <your-token>',
    skipDangerousModePermissionPrompt: true,
    skipWebFetchPreflight: true,
    env: { ANTHROPIC_BASE_URL: '${base}' }
  });
  merge(h + '/.claude.json', { hasCompletedOnboarding: true });
  try { fs.unlinkSync(h + '/.claude/.credentials.json'); } catch {}
"

claude`,
            },
          ],
        },
        {
          heading: 'What this does',
          text: '1. Merges apiKeyHelper + ANTHROPIC_BASE_URL into settings.json (bypasses OAuth — uses your proxy token directly)\n2. Enables skipWebFetchPreflight (domain verification goes through bridge)\n3. Sets hasCompletedOnboarding in .claude.json\n4. Removes old .credentials.json if present\nExisting settings are preserved — only proxy keys are updated.',
          blocks: [],
        },
        {
          text: 'The Claude Code VS Code extension reads the same ~/.claude/settings.json — no extra config needed. Just run the setup above and the extension will work.',
          blocks: [],
        },
        {
          heading: 'Revert to normal Claude login (remove proxy config)',
          blocks: [
            {
              label: 'Bash / Linux / macOS',
              copyCode: `node -e 'const fs=require("fs"),p=require("os").homedir()+"/.claude/settings.json";try{const j=JSON.parse(fs.readFileSync(p,"utf8"));delete j.apiKeyHelper;delete j.skipDangerousModePermissionPrompt;delete j.skipWebFetchPreflight;if(j.env)delete j.env.ANTHROPIC_BASE_URL;fs.writeFileSync(p,JSON.stringify(j,null,2),"utf8")}catch(e){console.warn(e.message)}' && claude login`,
              code: `node -e '
  const fs = require("fs");
  const p = require("os").homedir() + "/.claude/settings.json";
  try {
    const j = JSON.parse(fs.readFileSync(p, "utf8"));
    delete j.apiKeyHelper;
    delete j.skipDangerousModePermissionPrompt;
    delete j.skipWebFetchPreflight;
    if (j.env) delete j.env.ANTHROPIC_BASE_URL;
    fs.writeFileSync(p, JSON.stringify(j, null, 2), "utf8");
  } catch(e) { console.warn(e.message); }
' && claude login`,
            },
            {
              label: 'CMD (Windows)',
              copyCode: `node -e "const fs=require('fs'),p=process.env.USERPROFILE+'/.claude/settings.json';try{const j=JSON.parse(fs.readFileSync(p,'utf8'));delete j.apiKeyHelper;delete j.skipDangerousModePermissionPrompt;delete j.skipWebFetchPreflight;if(j.env)delete j.env.ANTHROPIC_BASE_URL;fs.writeFileSync(p,JSON.stringify(j,null,2),'utf8')}catch(e){console.warn(e.message)}" && claude login`,
              code: `node -e "
  const fs = require('fs');
  const p = process.env.USERPROFILE + '/.claude/settings.json';
  try {
    const j = JSON.parse(fs.readFileSync(p, 'utf8'));
    delete j.apiKeyHelper;
    delete j.skipDangerousModePermissionPrompt;
    delete j.skipWebFetchPreflight;
    if (j.env) delete j.env.ANTHROPIC_BASE_URL;
    fs.writeFileSync(p, JSON.stringify(j, null, 2), 'utf8');
  } catch(e) { console.warn(e.message); }
" && claude login`,
            },
          ],
        },
      ],
    },
    {
      id: 'opencode',
      label: 'OpenCode',
      hint: 'Replace <your-token> with a token from the table above.',
      sections: [
        {
          blocks: [
            {
              label: '~/.config/opencode/opencode.json',
              code: `{
  "provider": {
    "anthropic": {
      "options": {
        "apiKey": "<your-token>",
        "baseURL": "${base}"
      }
    }
  }
}`,
            },
          ],
        },
      ],
    },
    {
      id: 'cline',
      label: 'Cline',
      hint: 'Replace <your-token> with a token from the table above.',
      sections: [
        {
          text: 'VS Code Settings > Cline > API Configuration:',
          blocks: [
            {
              label: 'Settings',
              code: `Provider: Anthropic
Base URL: ${base}
API Key: <your-token>`,
            },
          ],
        },
      ],
    },
    {
      id: 'roo-code',
      label: 'Roo Code',
      hint: 'Replace <your-token> with a token from the table above.',
      sections: [
        {
          text: 'VS Code Settings > Roo Code > API Configuration:',
          blocks: [
            {
              label: 'Settings',
              code: `Provider: Anthropic
Base URL: ${base}
API Key: <your-token>`,
            },
          ],
        },
      ],
    },
    {
      id: 'kilo-code',
      label: 'Kilo Code',
      hint: 'Replace <your-token> with a token from the table above.',
      sections: [
        {
          text: 'VS Code Settings > Kilo Code > API Configuration:',
          blocks: [
            {
              label: 'Settings',
              code: `Provider: Anthropic
Base URL: ${base}
API Key: <your-token>`,
            },
          ],
        },
      ],
    },
  ]
}

/** Get a single guide by plugin id */
export function getGuideById(pluginId: string): Guide | undefined {
  return getConnectionGuides().find(g => g.id === pluginId)
}

/**
 * One-liner setup scripts for copy buttons — paste directly into terminal.
 * The merge logic handles UTF-8/UTF-16LE/BOM and preserves existing settings.
 */
export function getSetupScript(pluginId: string, token: string, shell: 'bash' | 'cmd'): string | null {
  if (pluginId !== 'claude-code') return null
  const base = getBase()

  // Shared merge logic (minified for inline use)
  // Reads existing JSON (handles UTF-16LE BOM from PowerShell), merges new keys, writes UTF-8
  const mergeFn = `function merge(p,d){let j={};if(fs.existsSync(p)){try{let r=fs.readFileSync(p);j=JSON.parse((r[0]===0xFF&&r[1]===0xFE?r.slice(2).toString('utf16le'):r.toString('utf8')).replace(/^\\uFEFF/,''))}catch(e){console.warn(e.message)}}const e=d.env;delete d.env;Object.assign(j,d);if(e)j.env=Object.assign(j.env||{},e);fs.writeFileSync(p,JSON.stringify(j,null,2),'utf8')}`

  if (shell === 'bash') {
    const h = 'require("os").homedir()'
    return `mkdir -p ~/.claude && node -e 'const fs=require("fs"),h=${h};${mergeFn};merge(h+"/.claude/settings.json",{apiKeyHelper:"echo ${token}",skipDangerousModePermissionPrompt:true,skipWebFetchPreflight:true,env:{ANTHROPIC_BASE_URL:"${base}"}});merge(h+"/.claude.json",{hasCompletedOnboarding:true});try{fs.unlinkSync(h+"/.claude/.credentials.json")}catch{}' && claude`
  }

  // CMD — double-quotes wrap the node -e string, inner strings use single quotes
  const h = 'process.env.USERPROFILE'
  return `mkdir "%USERPROFILE%\\.claude" 2>nul & node -e "const fs=require('fs'),h=${h};${mergeFn};merge(h+'/.claude/settings.json',{apiKeyHelper:'echo ${token}',skipDangerousModePermissionPrompt:true,skipWebFetchPreflight:true,env:{ANTHROPIC_BASE_URL:'${base}'}});merge(h+'/.claude.json',{hasCompletedOnboarding:true});try{fs.unlinkSync(h+'/.claude/.credentials.json')}catch{}" && claude`
}
