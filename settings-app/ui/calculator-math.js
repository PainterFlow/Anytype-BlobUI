(function (root) {
  "use strict";
  const UI = root.BlobUI = root.BlobUI || {};
  const OPERATIONS = { total: "Total", average: "Average", minimum: "Minimum", maximum: "Maximum" };
  const escapeXml = value => String(value).replace(/[&<>"']/g, character => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&apos;" })[character]);
  const token = (kind, fields = {}) => ({ kind, ...fields });

  function parseTokens(tokens) {
    if (!Array.isArray(tokens) || !tokens.length) throw Error("Enter a calculation.");
    if (tokens.length > 256) throw Error("Use at most 256 calculator terms.");
    tokens.forEach(item => {
      if (!item || typeof item !== "object") throw Error("This calculation contains an invalid term.");
      if (item.kind === "number") {
        if (typeof item.value !== "string" || item.value.length > 32) throw Error("This number is too long.");
        if (!/^(?:\d+(?:\.\d+)?|\.\d+)(?:[eE][+-]?\d+)?$/.test(item.value) || !Number.isFinite(Number(item.value))) throw Error("Complete the number.");
      } else if (item.kind === "aggregate") {
        if (typeof item.property !== "string" || !item.property || item.property.length > 256 || !Object.hasOwn(OPERATIONS, item.operation)) throw Error("Choose a supported numeric property and aggregation.");
      } else if (item.kind === "operator") {
        if (!['+', '-', '*', '/', '^'].includes(item.value)) throw Error("Choose a supported operator.");
      } else if (item.kind === "function") {
        if (!['sqrt', 'root'].includes(item.value)) throw Error("Choose a supported root function.");
      } else if (!['count', 'open', 'close', 'comma'].includes(item.kind)) throw Error("This calculation contains an unsupported term.");
    });

    let cursor = 0;
    const expect = kind => {
      if (tokens[cursor]?.kind !== kind) throw Error("Complete the calculation and check its parentheses.");
      return cursor++;
    };
    function expression(minimum = 0, depth = 0) {
      if (depth > 48) throw Error("This calculation is nested too deeply.");
      const start = cursor, item = tokens[cursor++];
      if (!item) throw Error("Complete the calculation.");
      let left;
      if (item.kind === "number") left = { type: "number", value: item.value, index: start, start, end: cursor };
      else if (item.kind === "aggregate") left = { type: "aggregate", property: item.property, operation: item.operation, index: start, start, end: cursor };
      else if (item.kind === "count") left = { type: "count", index: start, start, end: cursor };
      else if (item.kind === "operator" && (item.value === "-" || item.value === "+")) {
        const child = expression(20, depth + 1);
        left = { type: "unary", value: item.value, index: start, child, start, end: cursor };
      } else if (item.kind === "open") {
        const child = expression(0, depth + 1), closeIndex = expect("close");
        left = { type: "group", openIndex: start, closeIndex, child, start, end: cursor };
      } else if (item.kind === "function" && (item.value === "sqrt" || item.value === "root")) {
        const openIndex = expect("open"), value = expression(0, depth + 1);
        let degree = null, commaIndex = null;
        if (item.value === "root") { commaIndex = expect("comma"); degree = expression(0, depth + 1); }
        const closeIndex = expect("close");
        left = { type: "root", value: item.value, index: start, openIndex, closeIndex, commaIndex, child: value, degree, start, end: cursor };
      } else throw Error("Choose a number, property, or root.");

      while (tokens[cursor]?.kind === "operator") {
        const op = tokens[cursor].value, precedence = op === '+' || op === '-' ? 10 : op === '*' || op === '/' ? 20 : 30;
        if (precedence < minimum) break;
        const operatorIndex = cursor++;
        const right = expression(precedence + (op === '^' ? 0 : 1), depth + 1);
        left = { type: "binary", value: op, operatorIndex, left, right, start: left.start, end: cursor };
      }
      return left;
    }
    const tree = expression();
    if (cursor !== tokens.length) throw Error("Add an operator between terms, or check the parentheses.");
    return tree;
  }

  function groupedFraction(tokens) {
    if (!Array.isArray(tokens) || tokens.length < 7 || tokens[0]?.kind !== "open") return null;
    let depth = 0, topClose = -1;
    for (let index = 0; index < tokens.length; index++) {
      if (tokens[index].kind === "open") depth++;
      else if (tokens[index].kind === "close" && --depth === 0) { topClose = index; break; }
    }
    if (topClose < 2 || tokens[topClose + 1]?.kind !== "operator" || tokens[topClose + 1].value !== "/" || tokens[topClose + 2]?.kind !== "open") return null;
    depth = 0;
    let denominatorClose = -1;
    for (let index = topClose + 2; index < tokens.length; index++) {
      if (tokens[index].kind === "open") depth++;
      else if (tokens[index].kind === "close" && --depth === 0) { denominatorClose = index; break; }
    }
    if (denominatorClose !== tokens.length - 1) return null;
    return { numerator: tokens.slice(1, topClose), denominator: tokens.slice(topClose + 3, denominatorClose) };
  }

  function textForProperty(key, properties) {
    const property = (properties || []).find(item => item.key === key);
    return property ? property.name : "Unavailable property";
  }
  function latexTree(node, properties) {
    switch (node.type) {
      case "number": return node.value;
      case "aggregate": return `\\operatorname{${OPERATIONS[node.operation].toLowerCase()}}\\left(\\text{${escapeLatexText(textForProperty(node.property, properties))}}\\right)`;
      case "count": return "\\operatorname{count}\\left(\\text{cards}\\right)";
      case "unary": return `${node.value}${latexTree(node.child, properties)}`;
      case "group": return `\\left(${latexTree(node.child, properties)}\\right)`;
      case "root": return node.value === "sqrt"
        ? `\\sqrt{${latexTree(node.child, properties)}}`
        : `\\sqrt[${latexTree(node.degree, properties)}]{${latexTree(node.child, properties)}}`;
      case "binary": {
        if (node.value === "^") return `${latexTree(node.left, properties)}^{${latexTree(node.right, properties)}}`;
        if (node.value === "/") return `\\frac{${latexTree(node.left, properties)}}{${latexTree(node.right, properties)}}`;
        const symbol = node.value === "*" ? " \\cdot " : node.value;
        return `${latexTree(node.left, properties)}${symbol}${latexTree(node.right, properties)}`;
      }
      default: return "";
    }
  }
  function escapeLatexText(value) {
    return String(value).replace(/[\\{}%$&#_]/g, character => `\\${character}`);
  }
  function toLatex(tokens, properties = [], fraction = false) {
    if (fraction) {
      const top = tokens.numerator || [], bottom = tokens.denominator || [];
      return `\\frac{${top.length ? latexTree(parseTokens(top), properties) : ""}}{${bottom.length ? latexTree(parseTokens(bottom), properties) : ""}}`;
    }
    const parts = groupedFraction(tokens);
    if (parts) return `\\frac{${parts.numerator.length ? latexTree(parseTokens(parts.numerator), properties) : ""}}{${parts.denominator.length ? latexTree(parseTokens(parts.denominator), properties) : ""}}`;
    return latexTree(parseTokens(tokens), properties);
  }

  function attrIndex(index) { return ` data-token-index="${Number(index)}"`; }
  function caretMarkup() { return '<mspace class="formula-caret" width="0.12em" height="1em" aria-hidden="true"></mspace>'; }
  function mathNode(node, properties, cursor, active) {
    const mark = index => active && cursor === index ? caretMarkup() : "";
    switch (node.type) {
      case "number": return `${mark(node.index)}<mn${attrIndex(node.index)}>${escapeXml(node.value)}</mn>`;
      case "aggregate": {
        const label = textForProperty(node.property, properties);
        const operation = OPERATIONS[node.operation] || "Aggregate";
        return `${mark(node.index)}<mrow${attrIndex(node.index)}><mtext>${escapeXml(operation)}</mtext><mo>(</mo><mtext>${escapeXml(label)}</mtext><mo>)</mo></mrow>`;
      }
      case "count": return `${mark(node.index)}<mrow${attrIndex(node.index)}><mo>#</mo><mtext>cards</mtext></mrow>`;
      case "unary": return `${mark(node.index)}<mrow><mo${attrIndex(node.index)}>${node.value === '-' ? '−' : '+'}</mo>${mathNode(node.child, properties, cursor, active)}</mrow>`;
      case "group":
        if(["number","count","aggregate"].includes(node.child.type)) return `${mark(node.openIndex)}<mrow>${mathNode(node.child,properties,cursor,active)}${mark(node.closeIndex)}</mrow>`;
        return `${mark(node.openIndex)}<mrow><mo${attrIndex(node.openIndex)}>(</mo>${mathNode(node.child, properties, cursor, active)}${mark(node.closeIndex)}<mo${attrIndex(node.closeIndex)}>)</mo></mrow>`;
      case "root": {
        const prefix = mark(node.index) + mark(node.openIndex);
        const body = mathNode(node.child, properties, cursor, active);
        if (node.value === "sqrt") return `${prefix}<msqrt>${body}${mark(node.closeIndex)}</msqrt>`;
        const degree = mathNode(node.degree, properties, cursor, active);
        return `${prefix}<mroot>${body}${mark(node.commaIndex)}${degree}${mark(node.closeIndex)}</mroot>`;
      }
      case "binary": {
        if(node.value === "/" || node.value === "^") {const tag=node.value === "/"?"mfrac":"msup";return `<${tag}><mrow>${mathNode(node.left, properties,cursor,active)}</mrow><mrow>${mark(node.operatorIndex)}${mathNode(node.right, properties,cursor,active)}</mrow></${tag}>`;}
        const symbol = ({ "*": "×", "/": "÷", "-": "−" })[node.value] || node.value;
        return `<mrow>${mathNode(node.left, properties, cursor, active)}${mark(node.operatorIndex)}<mo${attrIndex(node.operatorIndex)}>${symbol}</mo>${mathNode(node.right, properties, cursor, active)}</mrow>`;
      }
      default: return "";
    }
  }
  function tokenFallback(tokens, cursor, active) {
    const pieces = [];
    const mark = index => active && cursor === index ? caretMarkup() : "";
    (tokens || []).forEach((item, index) => {
      pieces.push(mark(index));
      if (item.kind === "number") pieces.push(`<mn${attrIndex(index)}>${escapeXml(item.value)}</mn>`);
      else if (item.kind === "aggregate") pieces.push(`<mtext${attrIndex(index)}>${escapeXml(OPERATIONS[item.operation] || "Property")}</mtext>`);
      else if (item.kind === "count") pieces.push(`<mtext${attrIndex(index)}>cards</mtext>`);
      else if (item.kind === "function") pieces.push(`<mi${attrIndex(index)}>${item.value === "sqrt" ? '√' : 'ⁿ√'}</mi>`);
      else if (item.kind === "operator") pieces.push(`<mo${attrIndex(index)}>${escapeXml(({ "*": "×", "/": "÷", "-": "−" })[item.value] || item.value)}</mo>`);
      else pieces.push(`<mo${attrIndex(index)}>${escapeXml(({ open: "(", close: ")", comma: "," })[item.kind] || "")}</mo>`);
    });
    pieces.push(mark((tokens || []).length));
    return `<mrow>${pieces.join("") || '<mtext class="formula-placeholder">Enter a calculation</mtext>'}</mrow>`;
  }
  function toMathMLBody(tokens, properties = [], cursor = 0, active = true) {
    try {
      const tree = parseTokens(tokens);
      const inner = mathNode(tree, properties, cursor, active) + (active && cursor === tokens.length ? caretMarkup() : "");
      return `<mrow>${inner}</mrow>`;
    } catch (_) { return tokenFallback(tokens, cursor, active); }
  }
  function toMathML(tokens, properties = [], cursor = 0, active = true) {
    return `<math xmlns="http://www.w3.org/1998/Math/MathML">${toMathMLBody(tokens, properties, cursor, active)}</math>`;
  }

  function parseLatex(source, properties = []) {
    if (typeof source !== "string" || source.length > 4096) throw Error("LaTeX source is limited to 4,096 characters.");
    let index = 0;
    const skip = () => { while (/\s/.test(source[index] || "")) index++; };
    const fail = message => { throw Error(message); };
    const readCommand = () => {
      if (source[index] !== "\\") return "";
      index++;
      const start = index;
      while (/[A-Za-z]/.test(source[index] || "")) index++;
      if (start === index) {
        const escaped = source[index++];
        if (["{", "}", "%", "_", "#", "$", "&", "\\"].includes(escaped)) return escaped;
        fail("Unsupported LaTeX command.");
      }
      return source.slice(start, index);
    };
    const readGroup = () => {
      skip(); if (source[index] !== "{") fail("Expected a braced LaTeX argument.");
      index++; const start = index; let depth = 1, escaped = false;
      while (index < source.length) {
        const char = source[index++];
        if (escaped) { escaped = false; continue; }
        if (char === "\\") { escaped = true; continue; }
        if (char === "{") depth++;
        else if (char === "}" && --depth === 0) return source.slice(start, index - 1);
      }
      fail("Close the LaTeX brace.");
    };
    const readOptionalGroup = () => {
      skip(); if (source[index] !== "[") return null;
      index++; const start = index; let depth = 1;
      while (index < source.length) {
        const char = source[index++];
        if (char === "[") depth++;
        else if (char === "]" && --depth === 0) return source.slice(start, index - 1);
      }
      fail("Close the optional root degree.");
    };
    const readTextCommand = () => {
      skip(); const command = readCommand();
      if (command !== "text") fail("Property names must use a text command.");
      return readGroup().replace(/\\([\\{}%$&#_])/g, "$1");
    };
    const matchParen = (left, right) => {
      skip();
      if (source.startsWith("\\left", index)) { index += 5; skip(); }
      if (source[index] !== left) fail("Expected a parenthesis.");
      index++;
      return () => { skip(); if (source.startsWith("\\right", index)) { index += 6; skip(); } if (source[index] !== right) fail("Close the parenthesis."); index++; };
    };
    const precedence = op => op === "+" || op === "-" ? 10 : op === "*" || op === "/" ? 20 : op === "^" ? 30 : -1;
    function expression(minimum = 0, depth = 0) {
      if (depth > 48) fail("This calculation is nested too deeply.");
      skip(); let output = [];
      if (source[index] === "+" || source[index] === "-") {
        output.push(token("operator", { value: source[index++] }));
        output.push(...expression(20, depth + 1));
      } else if (source[index] === "(" || source.startsWith("\\left(", index)) {
        const close = matchParen("(", ")");
        output.push(token("open"), ...expression(0, depth + 1)); close(); output.push(token("close"));
      } else if (source[index] === "{") {
        output.push(token("open"), ...parseLatex(readGroup(), properties), token("close"));
      } else if (source[index] === "\\") {
        const command = readCommand();
        if (command === "sqrt") {
          const optional = readOptionalGroup();
          const body = readGroup();
          if (optional === null) output.push(token("function", { value: "sqrt" }), token("open"), ...parseLatex(body, properties), token("close"));
          else output.push(token("function", { value: "root" }), token("open"), ...parseLatex(body, properties), token("comma"), ...parseLatex(optional, properties), token("close"));
        } else if (command === "frac") {
          const numerator = parseLatex(readGroup(), properties), denominator = parseLatex(readGroup(), properties);
          output.push(token("open"), ...numerator, token("close"), token("operator", { value: "/" }), token("open"), ...denominator, token("close"));
        } else if (command === "operatorname") {
          const operation = readGroup().trim().toLowerCase();
          const close = matchParen("(", ")");
          const label = readTextCommand(); close();
          if (operation === "count") {
            if (label.toLowerCase() !== "cards") fail("Card count uses the label cards.");
            output.push(token("count"));
          } else {
            const match = Object.entries(OPERATIONS).find(([, display]) => display.toLowerCase() === operation);
            if (!match) fail("Choose Total, Average, Minimum, or Maximum for a property.");
            const matches = properties.filter(item => item.name === label);
            if (matches.length !== 1) fail(matches.length ? "This property name is ambiguous; insert it from the property picker." : "This property is unavailable in the current view.");
            output.push(token("aggregate", { operation: match[0], property: matches[0].key }));
          }
        } else if (command === "text") {
          const label = readGroup().replace(/\\([\\{}%$&#_])/g, "$1");
          if (label.toLowerCase() === "cards") output.push(token("count"));
          else fail("Properties need an aggregation command.");
        } else fail("Unsupported LaTeX command.");
      } else {
        const number = /^(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?/.exec(source.slice(index));
        if (number) { index += number[0].length; output.push(token("number", { value: number[0] })); }
        else fail("Enter a number, property, count, or supported LaTeX function.");
      }
      while (true) {
        skip(); const operatorStart = index; let op = source[index];
        if (source.startsWith("\\cdot", index) || source.startsWith("\\times", index)) { op = "*"; index += source.startsWith("\\cdot", index) ? 5 : 6; }
        else if (source.startsWith("\\div", index)) { op = "/"; index += 4; }
        else if (["+", "-", "*", "/", "^"].includes(op)) index++;
        else break;
        const rank = precedence(op);
        if (rank < minimum) { index = operatorStart; break; }
        output.push(token("operator", { value: op }), ...expression(rank + (op === "^" ? 0 : 1), depth + 1));
      }
      return output;
    }
    const tokens = expression(); skip();
    if (index !== source.length) fail("Check the LaTeX source near “" + source.slice(index, index + 12) + "”.");
    parseTokens(tokens);
    return tokens;
  }

  UI.CalculatorMath = { parseTokens, parseLatex, toLatex, toMathML, toMathMLBody, groupedFraction };
})(window);
