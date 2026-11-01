// Read the installed host without registering a plugin or touching personal data.
// Exercise its real mention dispatcher against an isolated stdio fixture.
const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const { spawn, spawnSync } = require('node:child_process');
const path = require('node:path');
const readline = require('node:readline');

if (process.argv[2] === '--fixture') {
  readline.createInterface({ input: process.stdin }).on('line', line => {
    const request = JSON.parse(line);
    const threadId = request.params._meta?.threadId;
    if(request.method==='resources/read') {
      const allowed=threadId==='project-a-chat';
      console.log(JSON.stringify(allowed ? {jsonrpc:'2.0',id:request.id,result:{contents:[{uri:request.params.uri,text:'Authorized A',mimeType:'text/plain'}]}} : {jsonrpc:'2.0',id:request.id,error:{code:-32000,message:'Read denied'}}));return;
    }
    const query=request.params.arguments.query;
    const terms=query.toLowerCase().split(/\s+/).filter(Boolean);
    const result={items:[{type:'resource_link',uri:'anyblob://objects/fixture/object',name:'猫 shared metadata'}].filter(item=>terms.every(term=>item.name.toLowerCase().includes(term)))};
    console.log(JSON.stringify({ jsonrpc: '2.0', id: request.id, result: { content: [], structuredContent: result } }));
  });
} else {
  main().catch(error => { console.error(error); process.exitCode = 1; });
}

async function main() {
  const archive = process.argv[2];
  assert(archive, 'Pass the installed desktop app.asar path.');
  const file = fs.openSync(archive, 'r');
  let source;
  let asset;
  try {
    const header = Buffer.alloc(16);
    fs.readSync(file, header, 0, 16, 0);
    const data = Buffer.alloc(header.readUInt32LE(12));
    fs.readSync(file, data, 0, data.length, 16);
    const base = 8 + header.readUInt32LE(4);
    function visit(files, prefix = '') {
      for (const [name, entry] of Object.entries(files)) {
        const path = `${prefix}/${name}`;
        if (entry.files) visit(entry.files, path);
        else if (!entry.unpacked && /\/webview\/assets\/app-initial-.*\.js$/.test(path)) {
          const bytes = Buffer.alloc(entry.size);
          fs.readSync(file, bytes, 0, bytes.length, base + Number(entry.offset));
          const text = bytes.toString('utf8');
          if (text.includes('async function sIn(')) { source = text; asset = path; }
        }
      }
    }
    visit(JSON.parse(data).files);
  } finally { fs.closeSync(file); }
  assert(source, 'Host changed. Inspect its dispatcher before updating this version-specific probe.');
  assert(source.includes('vSn=`openai/mentions`'));
  const start = source.indexOf('async function sIn(');
  const end = source.indexOf('async function cIn(', start);
  assert(end > start);
  // Feed the production executable's real discovery response into the installed selector.
  const executable=process.argv[3]||path.resolve(__dirname,'../dist/anyblob/bin/blobui-mcp.exe');
  const discovery=spawnSync(executable,[],{encoding:'utf8',windowsHide:true,env:{...process.env,ANYBLOB_BRIDGE_INSTANCE:'fixture-discovery'},
    input:[{jsonrpc:'2.0',id:1,method:'initialize',params:{protocolVersion:'2025-11-25'}},{jsonrpc:'2.0',id:2,method:'tools/list'}].map(v=>JSON.stringify(v)+'\n').join('')});
  assert.equal(discovery.status,0,discovery.stderr||discovery.error?.message);
  const [initialized,listed]=discovery.stdout.trim().split(/\r?\n/).map(line=>JSON.parse(line).result);
  assert.equal(initialized.capabilities.extensions['openai/mentions'].searchTool,'search_mentions');
  const searchTool=listed.tools.find(t=>t.name==='search_mentions');
  assert.equal(searchTool.annotations.readOnlyHint,true);
  assert(searchTool._meta.ui.visibility.includes('app'));
  const child = spawn(process.execPath, [__filename, '--fixture'], { stdio: ['pipe', 'pipe', 'inherit'], windowsHide: true });
  const pending = new Map();
  const deadline = setTimeout(() => { child.kill(); console.error('Fixture timed out.'); process.exitCode = 1; }, 10000);
  let sequence = 0;
  const lines = readline.createInterface({ input: child.stdout });
  lines.on('line', line => {
    const response = JSON.parse(line);
    const callback=pending.get(response.id);
    if(response.error)callback?.reject(Error(response.error.message));else callback?.resolve(response.result);
    pending.delete(response.id);
  });
  const observed = [];
  function call(options, query) {
    const request = { jsonrpc: '2.0', id: ++sequence, method: 'tools/call', params: { name: 'search_mentions', arguments: { query } } };
    if (options.threadId) request.params._meta = { threadId: options.threadId };
    observed.push(request);
    return new Promise((resolve,reject) => {
      pending.set(request.id, {resolve,reject});
      child.stdin.write(JSON.stringify(request) + '\n');
    });
  }
  const parser = { safeParse: data => ({ success: true, data }) };
  const context = vm.createContext({
    Promise, Error, Tr: { debug() {}, warning() {} }, ZNn: () => ({}),
    bIn: parser, _In: parser,
    // Capture exactly what the installed dispatcher supplies to its tool caller.
    cIn: (_scope, _host, _server, _tool, args, options) => call(options, args.query),
    QNn: (_labels,_meta,run)=>run(()=>{}),
    // Isolate the transport resolver. Never let the installed helper create fallback chats.
    uIn: (_scope,_host,threadId,_retry,_options,run)=>run(threadId,0),
    Hn: ()=>false,Yh:{},Pde:{parse:value=>value},
    tg: ()=>({sendRequest:async(method,params)=>{
      assert.equal(method,'mcpServer/resource/read');
      const request={jsonrpc:'2.0',id:++sequence,method:'resources/read',params:{uri:params.uri,_meta:{threadId:params.threadId}}};
      observed.push(request);
      return new Promise((resolve,reject)=>{pending.set(request.id,{resolve,reject});child.stdin.write(JSON.stringify(request)+'\n');});
    }}),
    Cy:()=>false,vSn:'openai/mentions',ySn:{},
    yMn:(capabilities,name)=>{const data=capabilities.extensions?.[name]??capabilities.experimental?.[name];return data?{success:typeof data.searchTool==='string'&&!!data.searchTool.trim(),data}:null;},
    lO:tools=>Object.values(tools),x1r:()=>null,wy:()=>null,hMn:()=>null,C1r:()=>null,S1r:tool=>tool.name,
  });
  vm.runInContext(source.slice(start, end), context, { timeout: 1000 });
  const readStart=source.indexOf('async function oIn(');
  assert(readStart>0&&readStart<start,'Host resource read helper changed. Inspect before updating this probe.');
  vm.runInContext(source.slice(readStart,start),context,{timeout:1000});
  const discoveryStart=source.indexOf('function m1r('),discoveryEnd=source.indexOf('function h1r(',discoveryStart);
  assert(discoveryStart>0&&discoveryEnd>discoveryStart,'Host mention provider discovery changed.');
  vm.runInContext(source.slice(discoveryStart,discoveryEnd),context,{timeout:1000});
  const server={name:'fixture',serverCapabilities:initialized.capabilities,serverInfo:initialized.serverInfo,tools:Object.fromEntries(listed.tools.map(tool=>[tool.name,tool]))};
  const discovered=context.m1r([server]);assert.equal(discovered.length,1);assert.equal(discovered[0].mentionSearchTool,'search_mentions');
  const invalid=structuredClone(server);invalid.tools.search_mentions.annotations.readOnlyHint=false;
  assert.equal(context.m1r([invalid]).length,0,'Writable search tools must not become mention providers');
  try {
    const providers = discovered;
    const results = await Promise.all(['project-a-chat', 'project-b-chat'].map(threadId =>
      context.sIn({ activeThreadId: threadId }, 'local', providers, { query: threadId==='project-a-chat'?'':'猫', threadId })));
    assert.equal(observed.length, 2);
    assert(observed.every(request => !request.params._meta?.threadId));
    assert(results.every(result => result[0].items.length === 1));
    for(const query of ['猫 ','猫 shared',' shared   metadata ']) {
      const found=await context.sIn({},'local',providers,{query});
      assert.equal(found[0].items.length,1);
      assert.equal(observed.at(-1).params.arguments.query,query,'Installed dispatcher must preserve spaces');
    }
    assert.equal((await call({threadId:'helper-chat'},'absent')).structuredContent.items.length,0);
    const uri=results[0][0].items[0].uri;
    const scope={get:()=>({})};
    const allowed=await context.oIn(scope,'local',{server:'fixture',uri,threadId:'project-a-chat'});
    assert.equal(allowed.contents[0].text,'Authorized A');
    await assert.rejects(context.oIn(scope,'local',{server:'fixture',uri,threadId:'project-b-chat'}),/Read denied/);
    await assert.rejects(context.oIn(scope,'local',{server:'fixture',uri}),/Read denied/);
    assert.deepEqual(observed.filter(r=>r.method==='resources/read').map(r=>r.params._meta.threadId),['project-a-chat','project-b-chat',undefined]);
    console.log(JSON.stringify({asset,extensionRecognized:true,productionCapabilityDiscoveryPassed:true,concurrentSearches:2,globalMetadataPassed:true,resourceReadBoundaryIdentityPassed:true,deniedReadsPassed:true,decision:'PASS',scope:'Installed webview dispatcher with isolated resolver and fixture transport; not full host-to-MCP transport'},null,2));
  } finally {
    clearTimeout(deadline);
    child.stdin.end();
    lines.close();
    child.kill();
  }
}
