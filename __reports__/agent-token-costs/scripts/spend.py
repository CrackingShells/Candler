import json,sys,glob,os
R={'opus':(4,20,8,0.2),'sonnet':(2,10,4,0.1),'haiku':(0.5,2.5,1,0.05),'fable':(4,20,8,0.2)}
def rate(m):
    for k,v in R.items():
        if k in (m or ''): return v
    return None
def cost(files):
    seen=set();tot={};unk=0
    for f in files:
        for line in open(f):
            try: d=json.loads(line)
            except: continue
            msg=d.get('message') or {}
            if not isinstance(msg,dict): continue
            u=msg.get('usage');mid=msg.get('id')
            if not u or not mid or mid in seen: continue
            seen.add(mid);r=rate(msg.get('model'))
            if not r: unk+=1;continue
            c=(u.get('input_tokens',0)*r[0]+u.get('output_tokens',0)*r[1]+u.get('cache_creation_input_tokens',0)*r[2]+u.get('cache_read_input_tokens',0)*r[3])/1e6
            k='main' if '/subagents/' not in f else 'sub'
            tot[k]=tot.get(k,0)+c
    return tot,len(seen),unk
D=sys.argv[1]
for s in sys.argv[2:]:
    t,n,u=cost([f'{D}/{s}.jsonl']+glob.glob(f'{D}/{s}/subagents/*.jsonl'))
    print(s[:8],{k:round(v,2) for k,v in t.items()},'total',round(sum(t.values()),2),'msgs',n,'unknown',u)
