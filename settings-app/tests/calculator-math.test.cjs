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
