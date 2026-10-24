const test=require('node:test');
const assert=require('node:assert/strict');
const vm=require('node:vm');
const fs=require('node:fs');
const path=require('node:path');
const window={};vm.runInNewContext(fs.readFileSync(path.join(__dirname,'../ui/calculator-math.js'),'utf8'),{window});
const math=window.BlobUI.CalculatorMath;
function value(node){
 if(node.type==='number')return Number(node.value);
 if(node.type==='group')return value(node.child);
 if(node.type==='unary')return (node.value==='-'?-1:1)*value(node.child);
 if(node.type==='root')return value(node.child)**(1/(node.degree?value(node.degree):2));
 if(node.type==='binary'){const a=value(node.left),b=value(node.right);return ({'+':()=>a+b,'-':()=>a-b,'*':()=>a*b,'/':()=>a/b,'^':()=>a**b})[node.value]();}
 throw Error('Unexpected term');
}
test('LaTeX roundtrip preserves calculator precedence and nested fraction values',()=>{
 for(const [source,expected]of [['2+3*4',14],['-2^2',-4],['2^3^2',512],['\\frac{8}{2}+{3}^{2}',13],['\\frac{1}{\\frac{2}{3}}',1.5],['\\sqrt[3]{8}+\\sqrt{9}',5]]){
  const tokens=math.parseLatex(source);assert.equal(value(math.parseTokens(tokens)),expected);
  const roundtrip=math.parseLatex(math.toLatex(tokens));assert.equal(value(math.parseTokens(roundtrip)),expected);
 }
 const markup=math.toMathML(math.parseLatex('2^3/4'));assert.match(markup,/<mfrac>/);assert.match(markup,/<msup>/);
});
test('LaTeX errors and unknown properties cannot become executable expressions',()=>{
 for(const source of ['\\frac{1}{','\\unknown{1}','alert(1)','\\operatorname{total}\\left(\\text{Missing}\\right)'])assert.throws(()=>math.parseLatex(source));
 assert.throws(()=>math.parseLatex('1'.repeat(4097)));
 const properties=[{key:'key',name:'A&B'}];
 const tokens=[{kind:'aggregate',property:'key',operation:'total'}];
 assert.equal(math.parseLatex(math.toLatex(tokens,properties),properties)[0].property,'key');
 assert.match(math.toMathML(tokens,properties),/A&amp;B/);
});

test('property references quote names and number duplicates by stable key for every aggregate',()=>{
 const name='A "quoted" {brace #1} \\path';
 const properties=[
  {key:'key-0',name,format:'text'},
  {key:'key-z',name,format:'number'},
  {key:'key-a',name,format:'checkbox'},
  {key:'key-m',name,format:'boolean'},
  {key:'unique',name:'Unique #2',format:'number'},
 ];
 const reversed=properties.slice().reverse();
 assert.equal(math.propertyReference('key-a',properties),'\\property{'+JSON.stringify(name)+'}#1');
 assert.equal(math.propertyReference('unique',properties),'\\property{"Unique #2"}');
 assert.equal(math.propertyReference('key-a',reversed),math.propertyReference('key-a',properties),'Duplicate ordinals do not depend on discovery order.');
 assert.throws(()=>math.propertyReference('key-0',properties),/unavailable/i,'Non-numeric properties do not alter selectable duplicate ordinals.');
 assert.throws(()=>math.parseLatex(`\\property{${JSON.stringify(name)}}`,properties),/ambiguous/i);
 for(const operation of ['total','average','minimum','maximum']){
  const tokens=[{kind:'aggregate',property:'key-m',operation}];
  const source=math.toLatex(tokens,properties);
  assert.match(source,/\\property\{"A .*brace #1.*"\}#2/);
  const parsed=math.parseLatex(source,properties);
  assert.equal(parsed[0].kind,'aggregate');
  assert.equal(parsed[0].property,'key-m');
  assert.equal(parsed[0].operation,operation);
  assert.equal(math.toLatex(tokens,reversed),source);
 }
 const escaped=math.toLatex([{kind:'aggregate',property:'unique',operation:'total'}],properties);
 assert.equal(math.parseLatex(escaped,properties)[0].property,'unique');
 assert.equal(math.toLatex([{kind:'aggregate',property:'unique',operation:'sum'}],properties),'\\property{"Unique #2"}{"total"}','Legacy aggregate tokens serialize with the canonical operation and explicit second argument.');
});

test('property command handles braces and escaped quotes inside its JSON name',()=>{
 const properties=[
  {key:'brace',name:'A}B {C',format:'number'},
  {key:'quote',name:'A "quote" \\ path #4',format:'number'},
 ];
 for(const property of properties){
  const source=`\\property{${JSON.stringify(property.name)}}{"sum"}`;
  const parsed=math.parseLatex(source,properties);
  assert.equal(parsed[0].property,property.key);
  assert.equal(parsed[0].operation,'total');
  assert.equal(math.toLatex(math.parseLatex(source,properties),properties),`\\property{${JSON.stringify(property.name)}}{"total"}`);
 }
 assert.throws(()=>math.parseLatex(`\\property{${JSON.stringify(properties[0].name)}}`,properties),/requires an aggregation/i);
});

test('nested formula groups keep JSON property-name braces and escapes intact',()=>{
 const name='A}B {C "quoted" \\path';
 const properties=[{key:'odd',name,format:'number'},{key:'total',name:'Total',format:'number'}];
 const source=`\\property{"Total"}{"sum"}{\\property{${JSON.stringify(name)}} + 1}`;
 const parsed=math.parseLatex(source,properties);
 assert.equal(parsed[0].calculation[0].property,'odd');
 const latex=math.toLatex(parsed,properties);
 assert.equal(math.toLatex(math.parseLatex(latex,properties),properties),latex);
 assert.match(math.toMathML(parsed,properties),/A\}B \{C &quot;quoted&quot;/);
});

test('optional root groups keep closing brackets inside quoted property names',()=>{
 const properties=[{key:'degree',name:'A]B',format:'number'},{key:'total',name:'Total',format:'number'}];
 const source='\\property{"Total"}{"sum"}{\\sqrt[\\property{"A]B"}]{8}}';
 const parsed=math.parseLatex(source,properties);
 assert.equal(parsed[0].calculation[0].kind,'function');
 assert.equal(parsed[0].calculation[0].value,'root');
 assert.equal(parsed[0].calculation[4].kind,'property');
 assert.equal(parsed[0].calculation[4].property,'degree');
 const latex=math.toLatex(parsed,properties);
 assert.equal(math.toLatex(math.parseLatex(latex,properties),properties),latex);
});

test('aggregate aliases canonicalize and unsupported text commands stay rejected',()=>{
 const properties=[{key:'amount',name:'Amount',format:'number'}];
 for(const [alias,canonical] of [['avg','average'],['average','average'],['sum','total'],['total','total'],['min','minimum'],['minimum','minimum'],['max','maximum'],['maximum','maximum'],['median','median']]){
  const parsed=math.parseLatex(`\\property{"Amount"}{${JSON.stringify(alias)}}`,properties);
  assert.equal(parsed[0].operation,canonical);
  assert.equal(math.toLatex(parsed,properties),`\\property{"Amount"}{${JSON.stringify(canonical)}}`);
 }
 assert.throws(()=>math.parseLatex('\\property{"Amount"}{["sum"]}',properties),/quoted operation name/i);
 assert.throws(()=>math.parseLatex('\\property{"Amount"}{4}',properties),/quoted operation name/i);
 for(const source of ['\\text{Amount}', '\\operatorname{sum}', '\\newline', '\\quad', '\\,']) assert.throws(()=>math.parseLatex(source,properties));
});

test('per-card formula arguments parse, validate, roundtrip, and render safely',()=>{
 const properties=[
  {key:'price',name:'Price & tax',format:'number'},
  {key:'qty',name:'Quantity',format:'number'},
  {key:'discount',name:'Discount',format:'number'},
 ];
 const source='\\property{"Price & tax"}{"sum"}{\\property{"Price & tax"} * \\property{"Quantity"} + \\property{"Discount"}}';
 const parsed=math.parseLatex(source,properties);
 assert.equal(parsed[0].kind,'aggregate');
 assert.equal(parsed[0].operation,'total');
 assert.deepEqual(JSON.parse(JSON.stringify(parsed[0].calculation)),[
  {kind:'property',property:'price'}, {kind:'operator',value:'*'}, {kind:'property',property:'qty'},
  {kind:'operator',value:'+'}, {kind:'property',property:'discount'},
 ]);
 const latex=math.toLatex(parsed,properties);
 assert.equal(math.toLatex(math.parseLatex(latex,properties),properties),latex);
 assert.match(math.toMathML(parsed,properties),/Price &amp; tax/);
 assert.match(math.toMathML(parsed,properties),/Quantity/);
 assert.throws(()=>math.parseLatex('\\property{"Price & tax"}',properties),/aggregation calculation/i);
 assert.throws(()=>math.parseTokens([{kind:'property',property:'price'}]),/aggregation calculation/i);
 assert.doesNotThrow(()=>math.parseTokens([{kind:'aggregate',property:'price',operation:'total',calculation:[{kind:'property',property:'price'}]}]),'A raw property is valid inside the calculation argument.');
 assert.doesNotThrow(()=>math.parseTokens(parsed));
});

test('saved aggregate tokens remain valid and nested scopes enforce shared limits',()=>{
 const properties=[{key:'price',name:'Price',format:'number'}];
 const old=[{kind:'aggregate',property:'price',operation:'sum'}];
 assert.equal(math.toLatex(old,properties),'\\property{"Price"}{"total"}');
 assert.deepEqual(JSON.parse(JSON.stringify(math.parseLatex(math.toLatex(old,properties),properties))),[{kind:'aggregate',property:'price',operation:'total'}]);
 let nested=[{kind:'property',property:'price'}];
 for(let i=0;i<49;i++) nested=[{kind:'aggregate',property:'price',operation:'total',calculation:nested}];
 assert.throws(()=>math.parseTokens(nested),/nested too deeply/i);
 assert.throws(()=>math.parseLatex('\\property{"Price"}{"sum"}{'.concat('\\property{"Price"}{"sum"}{'.repeat(48),'1','}'.repeat(49)),properties),/nested too deeply/i);
 const overBudget=[{kind:'aggregate',property:'price',operation:'total',calculation:Array.from({length:257},(_,i)=>i%2?{kind:'operator',value:'+'}:{kind:'number',value:'1'})}];
 assert.throws(()=>math.parseTokens(overBudget),/256 calculator terms/i);
});

test('boolean operators and conditionals roundtrip through LaTeX and MathML',()=>{
 for(const source of ['true && false || !false','1+2*3 >= 7 ? 10 : 0','false ? 1 : true ? 2 : 3','(true ? 10 : 0)+5','true == 1','1 != 2','1 <= 2','1 > 0','1 = 1','!2^2']) {
  const tokens=math.parseLatex(source), tree=math.parseTokens(tokens);
  const saved=math.toLatex(tokens), roundtrip=math.parseTokens(math.parseLatex(saved));
  const shape=n=>JSON.stringify(n,(k,v)=>v?.type==='group'?v.child:['index','start','end','operatorIndex','colonIndex','openIndex','closeIndex'].includes(k)?undefined:k==='value'&&v==='=='?'=':v);
  assert.equal(shape(roundtrip),shape(tree),saved);
  assert.doesNotMatch(math.toMathML(tokens),/formula-placeholder/);
 }
 const tree=math.parseTokens(math.parseLatex('1 < 2 && true || false ? 3 : 4'));
 assert.equal(tree.type,'conditional');assert.equal(tree.condition.value,'||');assert.equal(tree.condition.left.value,'&&');
 assert.match(math.toMathML(math.parseLatex('1 < 2 && !false')), /&lt;/);
 for(const source of ['true ? 1','true : 1','true ? : 2','1 ! 2','true &&','truefalse','true ? 1 : 2 : 3'])assert.throws(()=>math.parseLatex(source),source);
 assert.throws(()=>math.parseTokens([{kind:'boolean',value:'true'}]));
 assert.doesNotThrow(()=>math.parseLatex('\\neg false \\land true \\lor 1 \\ne 2 \\le 3 \\ge 0'));
});
