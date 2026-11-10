// Roadmap source of truth: validate docs/roadmap.json, render the human plan and
// sync one GitHub milestone per section plus one issue per feature, with
// explicit dependency links in every issue.
//
// Usage:
//   node script/roadmap.mjs                 # validate and print the summary
//   node script/roadmap.mjs --write         # regenerate docs/roadmap.md and docs/sections/*.md
//   node script/roadmap.mjs --apply         # create/update milestones and issues (needs gh auth)
//   node script/roadmap.mjs --apply --dry-run
//   node script/roadmap.mjs --apply --repo owner/name
import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const root = join(import.meta.dirname, '..');
const roadmap = JSON.parse(readFileSync(join(root, 'docs/roadmap.json'), 'utf8'));
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

const milestonesByKey = new Map();
const featuresById = new Map();
const blocks = new Map();

function validate() {
  if (!Array.isArray(roadmap.milestones) || roadmap.milestones.length === 0) {
    fail('roadmap.json has no milestones');
  }
  const groups = new Set((roadmap.groups || []).map((group) => group.id));
  for (const milestone of roadmap.milestones) {
    for (const field of ['id', 'key', 'title', 'summary']) {
      if (!milestone[field]) fail(`milestone ${milestone.key || '?'} is missing ${field}`);
    }
    if (!/^M\d+$/.test(milestone.id)) fail(`${milestone.key}: id must look like M12`);
    if (milestonesByKey.has(milestone.key)) fail(`duplicate milestone key ${milestone.key}`);
    milestonesByKey.set(milestone.key, milestone);
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
      if (featuresById.has(feature.id)) fail(`duplicate feature id ${feature.id}`);
      featuresById.set(feature.id, { feature, milestone });
      if (!Array.isArray(feature.acceptance) || feature.acceptance.length === 0) {
        fail(`${feature.id}: no acceptance criteria`);
      }
    }
  }
  for (const milestone of roadmap.milestones) {
    milestone.dependsOn ||= [];
    for (const key of milestone.dependsOn) {
      if (!milestonesByKey.has(key)) fail(`${milestone.key}: unknown milestone dependency ${key}`);
      if (key === milestone.key) fail(`${milestone.key}: depends on itself`);
    }
  }
  const milestoneVisiting = new Set();
  const milestoneVisited = new Set();
  const visitMilestone = (key) => {
    if (milestoneVisited.has(key)) return;
    if (milestoneVisiting.has(key)) fail(`milestone dependency cycle at ${key}`);
    milestoneVisiting.add(key);
    for (const dependency of milestonesByKey.get(key).dependsOn) visitMilestone(dependency);
    milestoneVisiting.delete(key);
    milestoneVisited.add(key);
  };
  for (const key of milestonesByKey.keys()) visitMilestone(key);
  for (const [id, { feature }] of featuresById) {
    feature.dependsOn ||= [];
    for (const dependency of feature.dependsOn) {
      if (!featuresById.has(dependency)) fail(`${id}: unknown feature dependency ${dependency}`);
      if (dependency === id) fail(`${id}: depends on itself`);
      if (!blocks.has(dependency)) blocks.set(dependency, []);
      blocks.get(dependency).push(id);
    }
  }
  const visiting = new Set();
  const visited = new Set();
  const visit = (id) => {
    if (visited.has(id)) return;
    if (visiting.has(id)) fail(`dependency cycle at ${id}`);
    visiting.add(id);
    for (const dependency of featuresById.get(id).feature.dependsOn) visit(dependency);
    visiting.delete(id);
    visited.add(id);
  };
  for (const id of featuresById.keys()) visit(id);
}

function counts() {
  const features = roadmap.milestones.reduce((total, m) => total + m.features.length, 0);
  return { milestones: roadmap.milestones.length, features };
}

function printSummary() {
  const { milestones, features } = counts();
  console.log(`${roadmap.product}: ${milestones} milestones, ${features} feature issues`);
  for (const milestone of roadmap.milestones) {
    const status = milestone.status || 'planned';
    const deps = (milestone.dependsOn || []).join(', ') || '-';
    console.log(
      `${milestone.id.padEnd(4)} ${milestone.key.padEnd(18)} ${String(milestone.features.length).padStart(2)} issues  ${status.padEnd(11)} deps: ${deps}`,
    );
  }
}

function list(values, prefix = '- ') {
  return (values || []).map((value) => `${prefix}${value}`).join('\n');
}

function featureLabel(id) {
  const entry = featuresById.get(id);
  return entry ? `${id} ${entry.feature.title}` : id;
}

function dependencyLine(feature) {
  if (!feature.dependsOn || feature.dependsOn.length === 0) return 'None; this is an entry point.';
  return feature.dependsOn.map(featureLabel).join(', ');
}

function sectionDoc(milestone) {
  const lines = [
    `# ${milestone.title}`,
    '',
    '<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->',
    '',
    `- Milestone: \`${milestone.id}\``,
    `- Section key: \`${milestone.key}\``,
    `- Group: \`${milestone.group}\``,
    `- Status: \`${milestone.status || 'planned'}\``,
    `- Depends on: ${(milestone.dependsOn || []).map((key) => `\`${key}\``).join(', ') || 'nothing'}`,
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
    lines.push(
      `### ${feature.id} — ${feature.title}`,
      '',
      feature.description,
      '',
      `Depends on: ${dependencyLine(feature)}`,
      '',
      'Acceptance:',
      '',
      list(feature.acceptance),
      '',
    );
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
    'The GitHub issue tracker is the working plan. Every feature issue carries its blocked-by and',
    'blocks links, upstream sources, section contract, acceptance criteria and verification.',
    '',
    '## Milestones',
    '',
    '| Milestone | Section | Group | Issues | Status | Depends on | Spec |',
    '| --- | --- | --- | --- | --- | --- | --- |',
  ];
  for (const milestone of roadmap.milestones) {
    lines.push(
      `| ${milestone.id} | ${milestone.title} | ${groups.get(milestone.group) || milestone.group} | ${milestone.features.length} | ${milestone.status || 'planned'} | ${(milestone.dependsOn || []).join(', ') || '-'} | [${milestone.key}](sections/${milestone.key}.md) |`,
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
    'Sync the GitHub milestones and issues:',
    '',
    '```console',
    'node script/roadmap.mjs --apply --repo owner/name --dry-run',
    'node script/roadmap.mjs --apply --repo owner/name',
    '```',
    '',
    'The sync is idempotent: milestones match by title and issues by feature id prefix. Existing',
    'issues are updated in place, so dependency links and acceptance criteria stay current.',
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

function patch(path, payload) {
  return gh(['api', path, '--method', 'PATCH', '--input', '-'], {
    input: JSON.stringify(payload),
  });
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

function issueLink(number, id) {
  const label = featureLabel(id);
  return number ? `#${number} ${label}` : label;
}

function issueBody(feature, milestone, numbers) {
  const blockedBy = feature.dependsOn || [];
  const blocking = blocks.get(feature.id) || [];
  const lines = [
    feature.description,
    '',
    '## Section',
    '',
    `- Milestone: ${milestone.id} ${milestone.title}`,
    `- Section key: \`${milestone.key}\``,
    `- Spec: docs/sections/${milestone.key}.md`,
    `- Upstream sources: ${(milestone.upstream || []).join(', ') || 'n/a'}`,
    '',
    '## Dependencies',
    '',
    'Blocked by:',
    ...(blockedBy.length
      ? blockedBy.map((id) => `- ${issueLink(numbers.get(id), id)}`)
      : ['- None; this is an entry point.']),
    '',
    'Blocks:',
    ...(blocking.length
      ? blocking.map((id) => `- ${issueLink(numbers.get(id), id)}`)
      : ['- None.']),
    '',
    '## Section contract',
    '',
    'UI:',
    ...(milestone.ui || []).map((item) => `- ${item}`),
    '',
    'Inputs:',
    ...(milestone.inputs || []).map((item) => `- ${item}`),
    '',
    'Outputs:',
    ...(milestone.outputs || []).map((item) => `- ${item}`),
    '',
    `Assets: ${milestone.assets || 'None.'}`,
    '',
    '## Acceptance',
    '',
    ...feature.acceptance.map((item) => `- [ ] ${item}`),
    '',
    '## Verification',
    '',
    ...(milestone.verification || []).map((item) => `- ${item}`),
    '',
    '## Agent notes',
    '',
    '- Port behavior from the upstream source listed above; do not infer rules from fixtures.',
    '- Keep `crates/core` free of GPUI and add formula tests before wiring the UI.',
    '- Use the shared components from M0; do not reimplement cards, tables, formula trees or task running.',
    '- Close this issue only when the acceptance list and the section verification pass.',
  ];
  return lines.join('\n');
}

function milestoneBody(milestone, milestoneNumbers, repo) {
  const deps = (milestone.dependsOn || []).map((key) => {
    const dependency = milestonesByKey.get(key);
    const number = milestoneNumbers.get(key);
    return number
      ? `[${dependency.id} ${dependency.title}](https://github.com/${repo}/milestone/${number})`
      : `${dependency.id} ${dependency.title}`;
  });
  return [
    milestone.summary,
    '',
    `Spec: docs/sections/${milestone.key}.md`,
    '',
    'Depends on:',
    ...(deps.length ? deps.map((item) => `- ${item}`) : ['- None.']),
    '',
    `Features: ${milestone.features.length}`,
  ].join('\n');
}

function trackerBody(numbers, milestoneNumbers, repo) {
  const lines = [
    roadmap.trackerIssue?.body || 'Implementation tracker.',
    '',
    '## Start here',
    '',
    '1. Finish M0 foundation issues in order; they provide the dashboard, save import and shared components.',
    '2. Finish M1 research; it exercises the shared save, formula tree and task runner end to end.',
    '3. Pick the next milestone whose dependencies are closed; follow each issue blocked-by list.',
    '',
    '## Milestones',
    '',
    '| Milestone | Issues | Depends on | Status |',
    '| --- | --- | --- | --- |',
  ];
  for (const milestone of roadmap.milestones) {
    const number = milestoneNumbers.get(milestone.key);
    const link = number
      ? `[${milestone.id} ${milestone.title}](https://github.com/${repo}/milestone/${number})`
      : `${milestone.id} ${milestone.title}`;
    lines.push(
      `| ${link} | ${milestone.features.length} | ${(milestone.dependsOn || []).join(', ') || '-'} | ${milestone.status || 'planned'} |`,
    );
  }
  lines.push('', '## Source documents', '', '- `docs/plan.md`', '- `docs/roadmap.md`', '- `docs/sections/`', '');
  return lines.join('\n');
}

function apply() {
  const repo = repository();
  const labels = new Map([
    ['roadmap', ['5319e7', 'Roadmap feature']],
    ['type:feature', ['1d76db', 'Feature issue']],
    ['type:tracking', ['b60205', 'Roadmap tracker']],
  ]);
  for (const milestone of roadmap.milestones) {
    labels.set(`area:${milestone.key}`, ['0e8a16', `Section ${milestone.key}`]);
  }
  const existingLabels = new Set();
  if (!dryRun || repoExists(repo)) {
    for (const label of apiAll(`repos/${repo}/labels`)) existingLabels.add(label.name);
  }
  for (const [name, [color, description]] of labels) {
    if (existingLabels.has(name)) continue;
    if (dryRun) console.log(`would create label ${name}`);
    else ensureLabel(repo, name, color, description);
  }

  const existingMilestones = new Map();
  const existingIssues = new Map();
  if (!dryRun || repoExists(repo)) {
    for (const milestone of apiAll(`repos/${repo}/milestones?state=all`)) {
      existingMilestones.set(milestone.title, milestone);
    }
    for (const issue of apiAll(`repos/${repo}/issues?state=all`)) {
      existingIssues.set(issue.title, issue);
    }
  }

  const milestoneNumbers = new Map();
  for (const milestone of roadmap.milestones) {
    const title = `${milestone.id} ${milestone.title}`;
    const existing = existingMilestones.get(title);
    if (existing) {
      milestoneNumbers.set(milestone.key, existing.number);
      const body = milestoneBody(milestone, milestoneNumbers, repo);
      if (!dryRun && existing.description !== body) {
        patch(`repos/${repo}/milestones/${existing.number}`, { description: body });
        console.log(`updated milestone ${title}`);
      }
    } else if (dryRun) {
      console.log(`would create milestone ${title}`);
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
          'description=',
        ]),
      );
      milestoneNumbers.set(milestone.key, created.number);
      patch(`repos/${repo}/milestones/${created.number}`, {
        description: milestoneBody(milestone, milestoneNumbers, repo),
      });
      console.log(`milestone ${title} -> #${created.number}`);
    }
  }

  // Create missing issues first so every dependency has a number, then patch
  // all bodies in one pass with resolved blocked-by/blocks links.
  for (const milestone of roadmap.milestones) {
    const milestoneTitle = `${milestone.id} ${milestone.title}`;
    for (const feature of milestone.features) {
      const title = `${feature.id} ${feature.title}`;
      if (existingIssues.has(title)) continue;
      if (dryRun) {
        console.log(`would create issue ${title}`);
        continue;
      }
      const args = [
        'issue',
        'create',
        '--repo',
        repo,
        '--title',
        title,
        '--body',
        feature.description,
        '--label',
        'type:feature',
        '--label',
        'roadmap',
        '--label',
        `area:${milestone.key}`,
      ];
      const milestoneNumber = milestoneNumbers.get(milestone.key);
      if (milestoneNumber) args.push('--milestone', milestoneTitle);
      const url = gh(args);
      const number = Number(url.split('/').pop());
      existingIssues.set(title, { number, body: feature.description });
      console.log(`issue ${title} -> ${url}`);
    }
  }

  const numbers = new Map();
  for (const [title, issue] of existingIssues) {
    const feature = roadmap.milestones
      .flatMap((milestone) => milestone.features)
      .find((candidate) => title === `${candidate.id} ${candidate.title}`);
    if (feature) numbers.set(feature.id, issue.number);
  }

  for (const milestone of roadmap.milestones) {
    for (const feature of milestone.features) {
      const title = `${feature.id} ${feature.title}`;
      const existing = existingIssues.get(title);
      if (!existing) continue;
      const body = issueBody(feature, milestone, numbers);
      if (dryRun) {
        console.log(`would update ${title} (${feature.dependsOn.length} blocked-by, ${(blocks.get(feature.id) || []).length} blocks)`);
      } else if (existing.body !== body) {
        patch(`repos/${repo}/issues/${existing.number}`, { body });
        console.log(`updated issue ${title}`);
      }
    }
  }

  const blockedBy = new Map();
  if (!dryRun || repoExists(repo)) {
    for (const issue of JSON.parse(
      gh(['issue', 'list', '--repo', repo, '--limit', '200', '--json', 'number,blockedBy']),
    )) {
      blockedBy.set(issue.number, new Set((issue.blockedBy?.nodes || []).map((node) => node.number)));
    }
  }
  for (const milestone of roadmap.milestones) {
    for (const feature of milestone.features) {
      if (!feature.dependsOn || feature.dependsOn.length === 0) continue;
      const issue = existingIssues.get(`${feature.id} ${feature.title}`);
      const dependencies = feature.dependsOn.map((id) => numbers.get(id)).filter(Boolean);
      if (!issue || dependencies.length === 0) continue;
      const missing = dependencies.filter((number) => !blockedBy.get(issue.number)?.has(number));
      if (missing.length === 0) continue;
      if (dryRun) {
        console.log(`would link ${feature.id} blocked by ${missing.map((n) => `#${n}`).join(', ')}`);
        continue;
      }
      gh([
        'issue',
        'edit',
        String(issue.number),
        '--repo',
        repo,
        '--add-blocked-by',
        missing.join(','),
      ]);
    }
  }

  if (roadmap.trackerIssue) {
    const title = roadmap.trackerIssue.title;
    const existing = existingIssues.get(title);
    const body = trackerBody(numbers, milestoneNumbers, repo);
    if (dryRun) {
      console.log(`would upsert tracker issue ${title}`);
    } else if (existing) {
      if (existing.body !== body) patch(`repos/${repo}/issues/${existing.number}`, { body });
      console.log(`tracker issue ${title} -> #${existing.number}`);
    } else {
      const args = [
        'issue',
        'create',
        '--repo',
        repo,
        '--title',
        title,
        '--body',
        body,
        '--label',
        'type:tracking',
        '--label',
        'roadmap',
      ];
      const milestoneNumber = milestoneNumbers.get('foundation');
      if (milestoneNumber) args.push('--milestone', 'M0 Foundation: dashboard, navigation, JSON import, assets');
      const url = gh(args);
      console.log(`tracker issue ${title} -> ${url}`);
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
