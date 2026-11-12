#!/usr/bin/env python3
"""Exercise the local Rust server, not a JavaScript policy reimplementation."""
import json, os, time, urllib.request, urllib.error
BASE=os.environ.get('POOL_DEMO_URL', 'http://127.0.0.1:8080')
def post(path, data):
    req=urllib.request.Request(BASE+'/api/'+path,json.dumps(data).encode(),{'Content-Type':'application/json'})
    with urllib.request.urlopen(req,timeout=5) as r: return json.load(r)
for attempt in range(40):
    try: post('scenarios',{}); break
    except (OSError,urllib.error.URLError): time.sleep(.25)
else: raise SystemExit('Start cargo run -p pool-demo -- serve first')
for name, expected in [('station',['32','20','44','4','0']),('gpu',['32','20','44','8','4','0']),('friday',['32','20','44','8','4','0']),('inventory',['32','8','0']),('hiring',['22','13','4','0']),('risk',['400','0','100','0','0'])]:
    p=post('reset',{'scenario':name})['pool']
    trace=post('trace',{'scenario':name})
    assert [s['decision']['after'] for s in trace['steps']]==expected
    for step in trace['steps']:
        q={**step['request'],'version':p['version'],'expires_at':int(time.time())+3600}
        r=post('submit',{'scenario':name,'request':q,'actor':step['actor']})
        assert r['decision']['allowed']==step['decision']['allowed']
        p=r['pool']
        if r['id'] is not None:p=post('lifecycle',{'scenario':name,'id':r['id'],'action':'complete'})['pool']
    assert p['values']==trace['steps'][-1]['pool']['values']
    print('PASS',name)
try:
    urllib.request.urlopen(urllib.request.Request(BASE+'/api/reset',b'{"scenario":"gpu"}',{'Content-Type':'application/json','Origin':'https://attacker.invalid'}))
    raise AssertionError('cross-origin write accepted')
except urllib.error.HTTPError as e: assert e.code==403
try: urllib.request.urlopen(BASE+'/Cargo.toml'); raise AssertionError('repository file exposed')
except urllib.error.HTTPError as e: assert e.code==404
print('PASS origin and static-file boundaries')
