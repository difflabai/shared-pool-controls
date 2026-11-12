"use strict";
const $=id=>document.getElementById(id);
const names=['station','gpu','friday','inventory','hiring','risk'];
const stories={
station:['Orbital commons','Every ship wants to dock','Atlas, Lyra and Vega share a station with 12 MW per docking window. Four MW are reserved for life support, leaving eight for docking. The lock can refuse a request even when its captain is authorized.','Squared unmet service targets plus a penalty for excess docking power. A separate hard ceiling of 8 MW per window protects the 4 MW life-support allocation.','Atlas reserves 4 MW and Lyra 2 MW. Vega requests 4 MW, which would take docking to 10 MW. WAIT TO DOCK: the request is denied and current bookings stay at 6 MW. Vega submits a new request against the next window; waiting itself grants no permission.'],
gpu:['Compute cooperative','Every team needs the GPU now','Three teams want the earliest GPU window. Each has approved work, but everyone shares the same headroom above a protected production-inference reserve.','Unfilled work squared + 10 × squared excess above 8 booked blocks per window. Hard ceiling: 12 ordinary blocks.','Research requests 4, Evaluation 2, and Data 4 blocks now: 10 in total. Local approvals miss the shared soft limit of 8. The lock offers Data 2 now and leaves the remainder for the next window.'],
friday:['Workshop network','Everyone promised Friday','Independent sales agents promise delivery against the same machines and dispatch slots. A profitable order still needs capacity the network can actually deliver.','Unfilled demand squared + 10 × squared excess above 8 machine bookings per date. Machine and dispatch constraints are checked together.','Three sales agents promise 10 lots on Friday. The shared gate limits peak loading and offers later dates. Rescheduling or reducing an order requires explicit customer consent.'],
inventory:['Service cooperative','Every branch hoards the scarce part','Branch A already has surplus parts. Branch B has a shortage. Independent agents share a depot that must keep four parts available for emergencies.','Sum of squared deviations from target positions of 4 parts at the depot and each branch. Incoming parts count in planning but are unavailable until received.','A requests two more parts for contingency stock. That would leave positions at 4, 8, 0 and raise energy from 24 to 32. The lock permits the same depot withdrawal for B instead, then a surplus transfer from A.'],
hiring:['Agent economy','Every agent hires more agents','A project funds research, implementation and an independent review. Agents may hire approved worker units, but cannot invent new funded milestones or consume the review earmark.','Squared funding deviations from Research 2, Implementation 3, Review 2. Three credits stay protected; two are earmarked for review.','Research commissions five credits of workers: within available discretionary funds, but beyond its useful scope. The five-credit request is denied; a revised two-credit request passes and keeps independent review funded. Workers cannot attest their own completion.'],
risk:['Multi-strategy fund','Everyone is diversified—until they need the same exit','Independent portfolio managers share stress-loss headroom and collateral. Local desk permissions cannot authorize the same aggregate risk capacity twice.','Squared stress-loss excess above soft limits [80, 60, 40], in $100k units. Hard limits [100, 80, 60]. No hedge benefit until confirmed fill.','The credit desk proposes a trade adding $4m selloff loss. Hard limits pass, but the soft budget is exceeded. Half size is allowed. A second desk cannot reuse that headroom; a hedge must actually fill before providing credit.']};
const tabLabels = {
  station: ['Orbital station', 'Power & life support'],
  gpu: ['Shared compute', 'GPU capacity'],
  friday: ['Friday promises', 'Production & delivery'],
  inventory: ['Scarce parts', 'Inventory & reserves'],
  hiring: ['Agents hiring agents', 'Budget & milestones'],
  risk: ['A shared risk pool', 'Exposure & collateral']
};
let scenario = 'station', pool, steps = [], cursor = 0, history = [], logs = [], offline = false, busy = false;

function node(tag, text, cls) {
  const n = document.createElement(tag);
  if (text !== undefined) n.textContent = String(text);
  if (cls) n.className = cls;
  return n;
}
// The Rust server only exists on a local host; elsewhere (GitHub Pages) replay recorded traces.
const localHosts = ['localhost', '127.0.0.1', '[::1]'];
const hasServer = localHosts.includes(location.hostname);
// Source checkout serves /ui/ with scenarios one level up; the Pages build has them beside index.html.
const scenarioRoot = /\/ui(\/|\/index\.html)?$/.test(location.pathname) ? '../scenarios/' : 'scenarios/';
async function api(path, data = {}) {
  if (!hasServer) throw Error('No local Rust server');
  const r = await fetch('/api/' + path, {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ scenario, ...data })
  });
  const v = await r.json();
  if (!r.ok || v.error) throw Error(v.error || r.status);
  return v;
}
function say(text, bad = false) {
  $('status').textContent = text;
  $('status').className = 'status-line' + (bad ? ' bad' : '');
}
function controls() {
  for (const id of ['next', 'run']) $(id).disabled = busy || !pool || cursor >= steps.length;
  for (const id of ['preview', 'submit', 'alternative', 'recipe', 'units', 'auto', 'shock']) {
    $(id).disabled = busy || offline || !pool;
  }
  $('reset').disabled = busy;
  $('station-next').disabled = busy || !pool || scenario !== 'station' || cursor >= steps.length;
  $('demo-station').disabled = busy || !pool;
  for (const b of $('tabs').children) b.disabled = busy;
}
async function guarded(fn) {
  if (busy) return;
  busy = true; controls();
  try { await fn(); } catch (e) { say(e.message, true); }
  finally { busy = false; controls(); }
}
function scoreFromTerms(values) {
  // Display only. Decisions always come from Rust or recorded Rust responses.
  return pool.config.terms.reduce((sum, t) => {
    let d = BigInt(values[t.metric]) - BigInt(t.target);
    if (t.shape === 'Excess' && d < 0n) d = 0n;
    if (t.shape === 'Deficit' && d > 0n) d = 0n;
    return sum + d * d * BigInt(t.weight);
  }, 0n).toString();
}
function actor() {
  $('actor').textContent = 'Simulated caller: ' + (pool.config.recipes[Number($('recipe').value)]?.actor || '');
}
function render() {
  $('version').textContent = 'Version ' + pool.version;
  $('current-energy').textContent = scoreFromTerms(pool.values);
  $('metrics').replaceChildren();
  pool.config.metrics.forEach((m, i) => {
    const item = node('div', undefined, 'metric');
    item.append(node('small', m.name), node('strong', pool.values[i]), node('small', m.unit + ' · limits ' + m.min + '–' + m.max));
    $('metrics').append(item);
  });
  const selected = $('recipe').value;
  $('recipe').replaceChildren();
  pool.config.recipes.forEach((r, i) => {
    const option = node('option', r.name); option.value = i; $('recipe').append(option);
  });
  if (selected && Number(selected) < pool.config.recipes.length) $('recipe').value = selected;
  actor();
  $('policy').textContent = JSON.stringify({ scenario, terms: pool.config.terms, metrics: pool.config.metrics }, null, 2);
  $('commitments').replaceChildren();
  for (const c of pool.records) {
    const row = node('tr');
    row.append(node('td', c.id), node('td', pool.config.recipes[c.recipe].name), node('td', c.units), node('td', c.status));
    const cell = node('td');
    if (!offline) {
      for (const [action, label] of [['complete', 'Complete'], ['unknown', 'Unknown'], ['cancel', 'Cancel'], ['not_executed', 'Not executed']]) {
        const b = node('button', label);
        b.disabled = ['Completed', 'Cancelled'].includes(c.status);
        b.setAttribute('aria-label', label + ' commitment ' + c.id);
        b.addEventListener('click', () => guarded(async () => {
          const r = await api('lifecycle', { id: c.id, action });
          pool = r.pool; history.push(scoreFromTerms(pool.values)); render();
          say('Evidence recorded: ' + label.toLowerCase() + '. The shared state has been refreshed.');
        }));
        cell.append(b);
      }
    } else cell.textContent = 'Recorded evidence';
    row.append(cell); $('commitments').append(row);
  }
  if (!pool.records.length) {
    const row = node('tr'), cell = node('td', 'No commitments yet. An allowed submission will appear here.');
    cell.colSpan = 5; row.append(cell); $('commitments').append(row);
  }
  const max = Math.max(1, ...history.map(Number));
  $('energy').replaceChildren();
  history.forEach((v, i) => {
    const b = node('div', undefined, 'bar');
    b.style.height = Math.max(3, Number(v) / max * 75) + 'px';
    b.title = (i === 0 ? 'Initial state' : 'After event ' + i) + ': ' + v;
    b.append(node('span', v)); $('energy').append(b);
  });
  $('progress').textContent = cursor + ' of ' + steps.length + ' scripted decisions';
  $('station-next').textContent = cursor === 3 && scenario === 'station' ? 'Request Vega docking in window 02 →' : cursor >= steps.length && scenario === 'station' ? 'Sequence complete' : 'Continue docking sequence →';
  $('next-action').textContent = cursor < steps.length
    ? pool.config.recipes[steps[cursor].request.recipe].name + ' · ' + steps[cursor].request.units + ' units requested'
    : 'Trace complete. Inspect the decisions or try another scenario.';
  $('shock').hidden = scenario !== 'risk' || offline;
  $('gpu-guide').hidden = scenario !== 'gpu';
  $('manual-note').textContent = offline
    ? 'This hosted page replays decisions generated by Rust. For custom requests, run the local Rust lab described in the GitHub README.'
    : 'A suggestion changes nothing until you explicitly submit it.';
  controls();
}
function renderStation(decision, request) {
  const allowed = decision?.allowed;
  const denied = decision && !allowed;
  const window = request ? request.recipe % 2 : 0;
  const shipIndex = request ? Math.floor(request.recipe / 2) : 2;
  const shipName = ['Atlas', 'Lyra', 'Vega'][shipIndex];
  const booked = decision ? pool.values[3 + window] : 0;
  const phase = denied ? 'holding' : decision ? (window ? 'resumed' : 'admitted') : 'ready';
  $('orbital-scene').dataset.phase = phase;
  $('orbital-scene').dataset.waiting = ['atlas', 'lyra', 'vega'][shipIndex];
  const markerOffsets = [[-462, 0], [-234, -53], [0, 0]];
  document.querySelector('.holding-marker').setAttribute('transform', 'translate(' + markerOffsets[shipIndex].join(' ') + ')');
  $('orbital-scene').classList.toggle('atlas-admitted', !!decision && pool.values[0] > 0);
  $('orbital-scene').classList.toggle('lyra-admitted', !!decision && pool.values[1] > 0);
  $('orbital-scene').classList.toggle('vega-admitted', !!decision && pool.values[2] > 0);
  $('atlas-status').textContent = decision && pool.values[0] > 0 ? 'RESERVED / W01' : 'APPROACHING';
  $('lyra-status').textContent = decision && pool.values[1] > 0 ? 'RESERVED' : 'APPROACHING';
  $('vega-status').textContent = denied ? 'HOLD POSITION' : decision && pool.values[2] > 0 ? 'RESERVED / W02' : 'APPROACHING';
  if (denied) $(['atlas-status', 'lyra-status', 'vega-status'][shipIndex]).textContent = 'HOLD POSITION';
  $('window-label').textContent = 'WINDOW 0' + (window + 1);
  $('docking-power').textContent = booked;
  [...$('power-cells').children].forEach((cell, i) => cell.classList.toggle('booked', i < 8 && i < booked));
  $('power-cells').setAttribute('aria-label', booked + ' of 8 MW docking power booked; 4 MW life support protected');
  $('flight-label').textContent = denied ? shipName + ' / request denied by contract' : decision ? 'Reservation admitted by contract' : 'Awaiting requests';
  $('flight-command').textContent = denied ? 'WAIT TO DOCK' : decision ? 'CLEARED TO RESERVE' : 'CLEARANCE REQUIRED';
  $('flight-detail').textContent = denied
    ? (decision.reason === 'Bounds' ? booked + ' + ' + request.units + ' MW requested. Only 8 MW is available for docking. ' : 'The request fails the ' + decision.reason + ' policy check. ') + shipName + ' holds; the ledger is unchanged.'
    : decision ? 'Window 0' + (window + 1) + ': ' + booked + ' MW reserved. A fresh request passed the shared policy. Dispatch remains a separate step.'
    : '12 MW total. 4 MW protected. Every ship must pass the same gate.';
}
function explanation(d, preview) {
  if (d.allowed) return preview
    ? 'The current policy permits this quantity. Previewing reserves nothing; submit explicitly to commit it.'
    : 'The commitment fits the current policy. Its resources now count against the shared pool.';
  return {
    Energy: 'The proposed commitment would make the configured imbalance worse. The shared state stays unchanged; a smaller or different request may fit.',
    Bounds: 'A resource limit or protected reserve would be violated. A better score elsewhere cannot override this limit.',
    Prerequisite: 'Required work has not yet been independently confirmed complete. The reserved review budget stays protected.',
    Stale: 'Another action changed the pool. Refresh the state and review this request again.',
    SnapshotExpired: 'The risk snapshot is too old. Fresh authoritative observations are required before taking more action.',
    Unauthorized: 'This caller is not authorized for the selected recipe.',
    Expired: 'The request is outside its allowed time window.'
  }[d.reason] || 'The contract did not admit this request. No resources were reserved.';
}
function show(result, request, preview = false) {
  const d = result.decision;
  if (!d) return;
  if (scenario === 'station' && !preview) renderStation(d, request);
  $('raw').textContent = JSON.stringify(result, null, 2);
  const box = node('div', undefined, 'decision' + (d.allowed ? '' : ' denied'));
  box.append(node('strong', (preview ? 'Preview: ' : '') + (d.allowed ? 'Allowed' : (scenario === 'station' ? 'WAIT TO DOCK · ' : 'Denied · ') + d.reason)),
    node('p', 'Energy ' + d.before + ' → ' + (d.after ?? 'unavailable')),
    node('p', explanation(d, preview), 'decision-explanation'));
  $('decision').replaceChildren(box);
  logs.unshift({ allowed: d.allowed, preview, name: pool.config.recipes[request.recipe].name, units: request.units, before: d.before, after: d.after });
  $('log').replaceChildren(...logs.map(entry => {
    const row = node('div', undefined, 'log-row');
    row.append(node('span', entry.preview ? 'PREVIEW' : entry.allowed ? 'ALLOW' : 'DENY', 'log-state' + (entry.allowed ? '' : ' denied')),
      node('span', entry.name + ' × ' + entry.units), node('span', entry.before + ' → ' + (entry.after ?? '—'), 'log-energy'));
    return row;
  }));
}
function req() {
  return { recipe: Number($('recipe').value), units: Number($('units').value), version: pool.version, expires_at: Math.floor(Date.now() / 1000) + 3600 };
}
async function next() {
  if (cursor >= steps.length) return;
  const step = steps[cursor];
  if (offline) {
    pool = step.pool; show(step, step.request); cursor++;
  } else {
    const q = { ...step.request, version: pool.version, expires_at: Math.floor(Date.now() / 1000) + 3600 };
    const r = await api('submit', { request: q, actor: step.actor });
    pool = r.pool; show(r, q); cursor++;
    if (r.id !== null && $('auto').checked) {
      try { pool = (await api('lifecycle', { id: r.id, action: 'complete' })).pool; }
      catch (e) { render(); throw e; }
    }
  }
  history.push(scoreFromTerms(pool.values)); render();
  say(cursor === steps.length ? 'All scripted requests evaluated. The decision trail shows what changed—and what was kept out.' : 'Decision recorded. Continue to see how the next request affects the same pool.');
}
async function reset() {
  cursor = 0; history = []; logs = [];
  $('log').replaceChildren(node('p', 'Each request leaves an explanation here.', 'empty-state'));
  $('decision').replaceChildren(node('p', 'Run the first decision to see what the lock checks.', 'empty-state'));
  $('raw').textContent = 'Run a scenario decision to inspect its response.';
  if (scenario === 'station') renderStation();
  const s = stories[scenario];
  ['category', 'title', 'story', 'formula', 'baseline'].forEach((id, i) => $(id).textContent = s[i]);
  $('scenario-number').textContent = String(names.indexOf(scenario) + 1).padStart(2, '0');
  $('scenario-panel').setAttribute('aria-labelledby', 'tab-' + scenario);
  try {
    pool = (await api('reset')).pool; steps = (await api('trace')).steps; offline = false;
  } catch (_) {
    const r = await fetch(scenarioRoot + scenario + '/trace.json');
    if (!r.ok) throw Error('The scenario could not load. Serve the repository root for static replay, or start the local Rust demo server.');
    const trace = await r.json(); pool = trace.initial; steps = trace.steps; offline = true;
  }
  $('mode').textContent = offline ? 'Rust-generated replay' : 'Live local Rust simulation';
  $('recipe').value = '0'; $('units').value = scenario === 'risk' || scenario === 'inventory' ? '2' : '4';
  history.push(scoreFromTerms(pool.values)); render();
  say('Ready. ' + (offline ? 'Step through recorded decisions from the Rust kernel.' : 'Each request is evaluated by the local Rust kernel.'));
  for (const b of $('tabs').children) {
    const selected = b.dataset.name === scenario;
    b.setAttribute('aria-selected', selected ? 'true' : 'false'); b.tabIndex = selected ? 0 : -1;
  }
}
for (const [i, name] of names.entries()) {
  const b = node('button');
  b.append(node('span', String(i + 1).padStart(2, '0'), 'tab-index'), node('span', tabLabels[name][0]), node('small', tabLabels[name][1]));
  b.dataset.name = name; b.id = 'tab-' + name;
  b.setAttribute('role', 'tab'); b.setAttribute('aria-controls', 'scenario-panel');
  b.setAttribute('aria-selected', name === scenario ? 'true' : 'false'); b.tabIndex = name === scenario ? 0 : -1;
  b.addEventListener('click', () => guarded(async () => { scenario = name; await reset(); }));
  b.addEventListener('keydown', event => {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key) || busy) return;
    event.preventDefault();
    const nextIndex = event.key === 'Home' ? 0 : event.key === 'End' ? names.length - 1 : (i + (event.key === 'ArrowRight' ? 1 : -1) + names.length) % names.length;
    guarded(async () => { scenario = names[nextIndex]; await reset(); })
      .then(() => $('tab-' + scenario).focus());
  });
  $('tabs').append(b);
}
$('demo-station').onclick = () => guarded(async () => {
  scenario = 'station';
  await reset();
  if (window.matchMedia('(max-width: 760px)').matches) {
    $('orbital-scene').scrollIntoView({ block: 'start' });
  }
  // Replay the two admissions and the denied request using the same Rust path.
  for (let i = 0; i < 3; i++) {
    await next();
    if (i < 2 && !window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
  }
  say('WAIT TO DOCK. Vega’s request was denied. Continue to submit a new request for the next window.');
});
$('station-next').onclick = () => guarded(next);
$('reset').onclick = () => guarded(reset);
$('next').onclick = () => guarded(next);
$('run').onclick = () => guarded(async () => { while (cursor < steps.length) await next(); });
$('recipe').onchange = actor;
for (const action of ['preview', 'submit']) $(action).onclick = () => guarded(async () => {
  const q = req(); const actor = pool.config.recipes[q.recipe].actor;
  const r = await api(action, { request: q, actor });
  pool = r.pool; show(r, q, action === 'preview');
  if (action === 'submit') history.push(scoreFromTerms(pool.values));
  render(); say(action === 'preview' ? 'Preview only. The pool has not changed.' : 'Request evaluated. Inspect the decision above.');
});
$('alternative').onclick = () => guarded(async () => {
  const q = req();
  const r = await api('alternative', { request: q, actor: pool.config.recipes[q.recipe].actor });
  if (r.request) {
    $('units').value = r.request.units;
    say('Suggested quantity: ' + r.request.units + '. Review and submit it explicitly. Nothing has been reserved.');
  } else say('No allowed smaller quantity. Try another approved action or wait for resources.', true);
});
$('shock').onclick = () => guarded(async () => {
  if (scenario !== 'risk' || offline) return;
  const values = [...pool.values]; values[0] += 30;
  pool = (await api('observe', { values })).pool;
  history.push(scoreFromTerms(pool.values)); render();
  say('External market observation recorded. This is a new measured baseline; energy can rise on external events.');
});
guarded(reset);


// Progressive presentation only. Headings, controls and decision commands remain immediate.
(function setupReveals() {
  const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');
  const revealAll = $('reveal-all');
  if (!('IntersectionObserver' in window) || reducedMotion.matches) return;
  const groups = document.querySelectorAll('.hero-copy, .orbital-scene, .premise-heading, .mechanism article, .energy-note, .deploy-guide, .section-heading, .demo-guide > div, .technical-points article, .evidence-grid article, .review-status > div, .near-boundary, .closing');
  const timers = new Map();
  const clear = group => {
    (timers.get(group) || []).forEach(clearTimeout);
    timers.delete(group);
  };
  const reveal = group => {
    clear(group);
    group.classList.add('summary-visible', 'details-visible');
    observer.unobserve(group);
  };
  const schedule = (group, engaged = false) => {
    if (group.classList.contains('details-visible')) return;
    if (timers.has(group) && !engaged) return;
    clear(group);
    if (engaged) group.classList.add('summary-visible');
    timers.set(group, [
      setTimeout(() => group.classList.add('summary-visible'), engaged ? 0 : 900),
      setTimeout(() => reveal(group), engaged ? 850 : 2600)
    ]);
  };
  const observer = new IntersectionObserver(entries => {
    for (const entry of entries) {
      if (entry.isIntersecting) schedule(entry.target);
      else clear(entry.target);
    }
  }, { threshold: 0.12 });
  for (const group of groups) {
    const supporting = group.querySelectorAll('[data-detail], :scope > p:not(.eyebrow), :scope > div > p');
    if (!supporting.length) continue;
    supporting.forEach((element, index) => {
      element.classList.add('supporting-detail');
      if (!element.dataset.detail) element.dataset.detail = index === 0 ? '1' : '2';
    });
    // Evidence links remain usable; keyboard focus reveals the whole group immediately.
    group.classList.add('reveal-group');
    group.addEventListener('pointerenter', event => {
      if (event.pointerType !== 'touch') schedule(group, true);
    });
    group.addEventListener('focusin', () => reveal(group));
    observer.observe(group);
  }
  const showEverything = () => {
    groups.forEach(reveal);
    observer.disconnect();
    revealAll.textContent = 'All details visible ✓';
    revealAll.disabled = true;
  };
  revealAll.hidden = false;
  revealAll.addEventListener('click', showEverything);
  reducedMotion.addEventListener('change', event => { if (event.matches) showEverything(); });
})();
