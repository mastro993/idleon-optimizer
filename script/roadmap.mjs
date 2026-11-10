// Roadmap source of truth: validate docs/roadmap.json, render the human plan and
// sync one GitHub milestone per section plus one issue per feature.
//
// Usage:
//   node script/roadmap.mjs                 # validate and print the summary
//   node script/roadmap.mjs --write         # regenerate docs/roadmap.md and docs/sections/*.md
//   node script/roadmap.mjs --apply         # create missing milestones/issues (needs gh auth)
//   node script/roadmap.mjs --apply --dry-run
//   node script/roadmap.mjs --apply --repo owner/name
import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

const root = join(import.meta.dirname, '..');
const roadmapPath = join(root, 'docs/roadmap.json');
const roadmap = JSON.parse(readFileSync(roadmapPath, 'utf8'));
const argv = process.argv.slice(2);
const flag = (name, fallback = null) => {
  const index = argv.indexOf(name);
  return index >= 0 ? argv[index + 1] : fallback;
};
const has = (name) => argv.includes(name);
const dryRun = has('--dry-run');

function fail(message) {
  console.error(message);
  process.exit(1);
}

function validate() {
  if (!Array.isArray(roadmap.milestones) || roadmap.milestones.length === 0) {
    fail('roadmap.json has no milestones');
  }
  const groups = new Set((roadmap.groups || []).map((group) => group.id));
  const milestoneKeys = new Set();
  const featureIds = new Set();
  for (const milestone of roadmap.milestones) {
    for (const field of ['id', 'key', 'title', 'summary']) {
      if (!milestone[field]) fail(`milestone ${milestone.key || '?'} is missing ${field}`);
    }
    if (!/^M\d+$/.test(milestone.id)) fail(`${milestone.key}: id must look like M12`);
    if (milestoneKeys.has(milestone.key)) fail(`duplicate milestone key ${milestone.key}`);
    milestoneKeys.add(milestone.key);
    if (groups.size > 0 && !groups.has(milestone.group)) {
      fail(`${milestone.key}: unknown group ${milestone.group}`);
    }
    if (!Array.isArray(milestone.features) || milestone.features.length === 0) {
      fail(`${milestone.key}: no features`);
    }
    for (const feature of milestone.features) {
      for (const field of ['id', 'title', 'description']) {
        if (!feature[field]) fail(`${milestone.key}: feature is missing ${field}`);
      }
      if (featureIds.has(feature.id)) fail(`duplicate feature id ${feature.id}`);
      featureIds.add(feature.id);
      if (!Array.isArray(feature.acceptance) || feature.acceptance.length === 0) {
        fail(`${feature.id}: no acceptance criteria`);
      }
    }
  }
}

function counts() {
  const features = roadmap.milestones.reduce((total, m) => total + m.features.length, 0);
  return { milestones: roadmap.milestones.length, features };
}

function printSummary() {
  const { milestones, features } = counts();
  console.log(`${roadmap.product}: ${milestones} milestones, ${features} feature issues`);
  for (const milestone of roadmap.milestones) {
    const done = milestone.status === 'done' ? 'done' : milestone.status || 'planned';
    console.log(`${milestone.id.padEnd(4)} ${milestone.key.padEnd(18)} ${String(milestone.features.length).padStart(2)} issues  ${done}`);
  }
}

function list(values, prefix = '- ') {
  return (values || []).map((value) => `${prefix}${value}`).join('\n');
}

function sectionDoc(milestone) {
  const lines = [
    `# ${milestone.title}`,
    '',
    `<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->`,
    '',
    `- Milestone: \`${milestone.id}\``,
    `- Section key: \`${milestone.key}\``,
    `- Group: \`${milestone.group}\``,
    `- Status: \`${milestone.status || 'planned'}\``,
    '',
    milestone.summary,
    '',
    '## Upstream sources',
    '',
    list(milestone.upstream) || '- none',
    '',
    '## UI',
    '',
    list(milestone.ui) || '- none',
    '',
    '## Inputs',
    '',
    list(milestone.inputs) || '- none',
    '',
    '## Outputs',
    '',
    list(milestone.outputs) || '- none',
    '',
    '## Assets',
    '',
    milestone.assets || 'None.',
    '',
    '## Feature issues',
    '',
  ];
  for (const feature of milestone.features) {
    lines.push(`### ${feature.id} — ${feature.title}`, '', feature.description, '', 'Acceptance:', '');
    lines.push(list(feature.acceptance));
    lines.push('');
  }
  lines.push('## Verification', '', list(milestone.verification) || '- none', '');
  return lines.join('\n');
}

function roadmapDoc() {
  const { milestones, features } = counts();
  const groups = new Map((roadmap.groups || []).map((group) => [group.id, group.label]));
  const lines = [
    '# Roadmap and progress tracking',
    '',
    '<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->',
    '',
    `${roadmap.product} tracks ${milestones} section milestones and ${features} feature issues.`,
    `Source repository: ${roadmap.source}.`,
    '',
    '## Milestones',
    '',
    '| Milestone | Section | Group | Issues | Status | Spec |',
    '| --- | --- | --- | --- | --- | --- |',
  ];
  for (const milestone of roadmap.milestones) {
    lines.push(
      `| ${milestone.id} | ${milestone.title} | ${groups.get(milestone.group) || milestone.group} | ${milestone.features.length} | ${milestone.status || 'planned'} | [${milestone.key}](sections/${milestone.key}.md) |`,
    );
  }
  lines.push(
    '',
    '## Tracking',
    '',
    '`docs/roadmap.json` is the source of truth: one milestone per section and one issue per feature.',
    'Validate and render with:',
    '',
    '```console',
    'node script/roadmap.mjs --write',
    '```',
    '',
    'Create the GitHub milestones and issues once the repository has a remote:',
    '',
    '```console',
    'node script/roadmap.mjs --apply --repo owner/name --dry-run',
    'node script/roadmap.mjs --apply --repo owner/name',
    '```',
    '',
    'The sync is idempotent: milestones match by title and issues by their feature id prefix.',
    'Close an issue to update GitHub progress; update the milestone status in `roadmap.json` when a',
    'section is complete, then regenerate this page.',
    '',
  );
  return lines.join('\n');
}

function writeDocs() {
  const sectionsDir = join(root, 'docs/sections');
  mkdirSync(sectionsDir, { recursive: true });
  writeFileSync(join(root, 'docs/roadmap.md'), roadmapDoc());
  const index = [
    '# Section specifications',
    '',
    '<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->',
    '',
  ];
  for (const milestone of roadmap.milestones) {
    writeFileSync(join(sectionsDir, `${milestone.key}.md`), sectionDoc(milestone));
    index.push(`- [${milestone.id} ${milestone.title}](sections/${milestone.key}.md)`);
  }
  writeFileSync(join(sectionsDir, 'README.md'), index.join('\n') + '\n');
  console.log(`wrote docs/roadmap.md and ${roadmap.milestones.length} section specs`);
}

function gh(args, options = {}) {
  return execFileSync('gh', args, { encoding: 'utf8', ...options }).trim();
}

function repository() {
  if (roadmap.repo) return roadmap.repo;
  try {
    return gh(['repo', 'view', '--json', 'nameWithOwner', '--jq', '.nameWithOwner']);
  } catch {
    fail('no repository: pass --repo owner/name or set repo in docs/roadmap.json');
  }
}

function api(path) {
  return JSON.parse(gh(['api', path]));
}

function apiAll(path) {
  const all = [];
  for (let page = 1; ; page += 1) {
    const separator = path.includes('?') ? '&' : '?';
    const chunk = api(`${path}${separator}per_page=100&page=${page}`);
    all.push(...chunk);
    if (chunk.length < 100) break;
  }
  return all;
}

function repoExists(repo) {
  try {
    gh(['repo', 'view', repo, '--json', 'name']);
    return true;
  } catch {
    return false;
  }
}

function ensureLabel(repo, name, color, description) {
  try {
    gh(['label', 'create', name, '--repo', repo, '--color', color, '--description', description]);
    console.log(`label ${name}`);
  } catch (error) {
    if (!String(error.stderr || '').includes('already exists')) throw error;
  }
}

function apply() {
  const repo = repository();
  const labels = new Map([
    ['roadmap', ['5319e7', 'Roadmap feature']],
    ['type:feature', ['1d76db', 'Feature issue']],
  ]);
  for (const milestone of roadmap.milestones) {
    labels.set(`area:${milestone.key}`, ['0e8a16', `Section ${milestone.key}`]);
  }
  for (const [name, [color, description]] of labels) {
    if (dryRun) console.log(`would ensure label ${name}`);
    else ensureLabel(repo, name, color, description);
  }

  const existingMilestones = new Map();
  const existingIssues = new Map();
  if (!dryRun || repoExists(repo)) {
    for (const milestone of apiAll(`repos/${repo}/milestones?state=all`)) {
      existingMilestones.set(milestone.title, milestone.number);
    }
    for (const issue of apiAll(`repos/${repo}/issues?state=all`)) {
      existingIssues.set(issue.title, issue.number);
    }
  }

  for (const milestone of roadmap.milestones) {
    const title = `${milestone.id} ${milestone.title}`;
    let number = existingMilestones.get(title);
    if (!number) {
      const body = `${milestone.summary}\n\nSpec: docs/sections/${milestone.key}.md`;
      if (dryRun) {
        console.log(`would create milestone ${title}`);
        number = 0;
      } else {
        const created = JSON.parse(
          gh([
            'api',
            `repos/${repo}/milestones`,
            '--method',
            'POST',
            '-f',
            `title=${title}`,
            '-f',
            `description=${body}`,
          ]),
        );
        number = created.number;
        console.log(`milestone ${title} -> #${number}`);
      }
    }
    for (const feature of milestone.features) {
      const issueTitle = `${feature.id} ${feature.title}`;
      if (existingIssues.has(issueTitle)) {
        console.log(`issue exists ${issueTitle}`);
        continue;
      }
      const body = [
        feature.description,
        '',
        `Section: ${milestone.title} (${milestone.id})`,
        `Spec: docs/sections/${milestone.key}.md`,
        `Upstream: ${(milestone.upstream || []).join(', ') || 'n/a'}`,
        '',
        'Acceptance:',
        ...feature.acceptance.map((item) => `- [ ] ${item}`),
        '',
        `Verification: ${(milestone.verification || []).join('; ') || 'n/a'}`,
      ].join('\n');
      const args = [
        'issue',
        'create',
        '--repo',
        repo,
        '--title',
        issueTitle,
        '--body',
        body,
        '--label',
        'type:feature',
        '--label',
        'roadmap',
        '--label',
        `area:${milestone.key}`,
      ];
      if (number) args.push('--milestone', title);
      if (dryRun) {
        console.log(`would create issue ${issueTitle}`);
        continue;
      }
      const url = gh(args);
      console.log(`issue ${issueTitle} -> ${url}`);
    }
  }
}

validate();
if (has('--write')) writeDocs();
printSummary();
if (has('--apply')) apply();
if (!has('--write') && !has('--apply')) {
  console.log('\nRun --write to render docs, or --apply to sync GitHub milestones/issues.');
}
