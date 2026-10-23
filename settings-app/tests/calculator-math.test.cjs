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
 assert.equal(math.parseLatex('\\operatorname{average}\\left(\\text{Unique #2}\\right)',properties)[0].operation,'average','Legacy aggregation source remains readable.');
});

test('property command handles braces and escaped quotes inside its JSON name',()=>{
 const properties=[
  {key:'brace',name:'A}B {C',format:'number'},
  {key:'quote',name:'A "quote" \\ path #4',format:'number'},
 ];
 for(const property of properties){
  const source=`\\property{${JSON.stringify(property.name)}}`;
  assert.equal(math.parseLatex(source,properties)[0].property,property.key);
  assert.equal(math.toLatex(math.parseLatex(source,properties),properties),source);
 }
});
