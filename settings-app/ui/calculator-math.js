(function (root) {
  "use strict";
  const UI = root.BlobUI = root.BlobUI || {};
  const OPERATIONS = { total: "Total", sum: "Sum", average: "Average", avg: "Average", median: "Median", minimum: "Minimum", min: "Minimum", maximum: "Maximum", max: "Maximum" };
  const CANONICAL_OPERATION = { total: "total", sum: "total", average: "average", avg: "average", median: "median", minimum: "minimum", min: "minimum", maximum: "maximum", max: "maximum" };
  const escapeXml = value => String(value).replace(/[&<>"']/g, character => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&apos;" })[character]);
  const token = (kind, fields = {}) => ({ kind, ...fields });

  function parseTokens(tokens, context = {}) {
    const budget = { count: 0 };
    return parseTokensInternal(tokens, context.allowPerCard === true, budget, 0);
  }
  function parseTokensInternal(tokens, allowPerCard, budget, nesting) {
    if (!Array.isArray(tokens) || !tokens.length) throw Error("Enter a calculation.");
    if (nesting > 48) throw Error("This calculation is nested too deeply.");
    budget.count += tokens.length;
    if (budget.count > 256) throw Error("Use at most 256 calculator terms.");
    tokens.forEach(item => {
      if (!item || typeof item !== "object") throw Error("This calculation contains an invalid term.");
      if (item.kind === "number") {
        if (typeof item.value !== "string" || item.value.length > 32) throw Error("This number is too long.");
        if (!/^(?:\d+(?:\.\d+)?|\.\d+)(?:[eE][+-]?\d+)?$/.test(item.value) || !Number.isFinite(Number(item.value))) throw Error("Complete the number.");
      } else if (item.kind === "aggregate") {
        if (typeof item.property !== "string" || !item.property || item.property.length > 256 || !Object.hasOwn(CANONICAL_OPERATION, item.operation)) throw Error("Choose a supported numeric property and aggregation.");
      } else if (item.kind === "property") {
        if (!allowPerCard || typeof item.property !== "string" || !item.property || item.property.length > 256) throw Error("A per-card property requires an aggregation calculation.");
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
      if (depth + nesting > 48) throw Error("This calculation is nested too deeply.");
      const start = cursor, item = tokens[cursor++];
      if (!item) throw Error("Complete the calculation.");
      let left;
      if (item.kind === "number") left = { type: "number", value: item.value, index: start, start, end: cursor };
      else if (item.kind === "aggregate") {
        const calculation = item.calculation === undefined ? null : parseTokensInternal(item.calculation, true, budget, nesting + depth + 1);
        left = { type: "aggregate", property: item.property, operation: CANONICAL_OPERATION[item.operation], calculation, index: start, start, end: cursor };
      }
      else if (item.kind === "property") left = { type: "property", property: item.property, index: start, start, end: cursor };
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
      } else throw Error("Choose a number, aggregate, or root.");

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
  function isSelectableProperty(property) {
    return !!property && typeof property.key === "string" && typeof property.name === "string"
      && (property.format === undefined || ["number", "boolean", "checkbox"].includes(property.format));
  }
  function propertyReference(key, properties) {
    const property = (properties || []).find(item => isSelectableProperty(item) && item.key === key);
    if (!property) throw Error("This property is unavailable in the current view.");
    const duplicates = (properties || [])
      .filter(item => isSelectableProperty(item) && item.name === property.name)
      .sort((left, right) => left.key < right.key ? -1 : left.key > right.key ? 1 : 0);
    const ordinal = duplicates.findIndex(item => item.key === key);
    if (ordinal < 0) throw Error("This property has no stable key in the current view.");
    const encodedName = JSON.stringify(property.name);
    return `\\property{${encodedName}}${duplicates.length > 1 ? `#${ordinal + 1}` : ""}`;
  }
  function latexTree(node, properties) {
    switch (node.type) {
      case "number": return node.value;
      case "aggregate": return `${propertyReference(node.property, properties)}{${JSON.stringify(node.operation)}}${node.calculation ? `{${latexTree(node.calculation, properties)}}` : ""}`;
      case "property": return propertyReference(node.property, properties);
      case "count": return "\\count";
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
  function caretMarkup() { return '<mspace class="formula-caret" width="1px" height="0.85em" depth="0.15em" aria-hidden="true"></mspace>'; }
  function mathNode(node, properties, cursor, active) {
    const mark = index => active && cursor === index ? caretMarkup() : "";
    switch (node.type) {
      case "number": return `${mark(node.index)}<mn${attrIndex(node.index)}>${escapeXml(node.value)}</mn>`;
      case "aggregate": {
        const label = textForProperty(node.property, properties);
        const operation = OPERATIONS[node.operation] || "Aggregate";
        const calculation = node.calculation ? `<mo>[</mo>${mathNode(node.calculation, properties, cursor, active)}<mo>]</mo>` : "";
        return `${mark(node.index)}<mrow${attrIndex(node.index)}><mtext>${escapeXml(operation)}</mtext><mo>(</mo><mtext>${escapeXml(label)}</mtext>${calculation}<mo>)</mo></mrow>`;
      }
      case "property": return `${mark(node.index)}<mtext${attrIndex(node.index)}>${escapeXml(textForProperty(node.property, properties))}</mtext>`;
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
      else if (item.kind === "property") pieces.push(`<mtext${attrIndex(index)}>Property</mtext>`);
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

  function parseLatex(source, properties = [], context = {}) {
    return parseLatexInternal(source, properties, context.allowPerCard === true, 0);
  }
  function parseLatexInternal(source, properties, allowPerCard, nesting) {
    if (nesting > 48) throw Error("This calculation is nested too deeply.");
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
      index++; const start = index; let depth = 1, escaped = false, inString = false;
      while (index < source.length) {
        const char = source[index++];
        if (inString) {
          if (escaped) { escaped = false; continue; }
          if (char === "\\") { escaped = true; continue; }
          if (char === '"') inString = false;
          continue;
        }
        if (escaped) { escaped = false; continue; }
        if (char === "\\") { escaped = true; continue; }
        if (char === '"') { inString = true; continue; }
        if (char === "{") depth++;
        else if (char === "}" && --depth === 0) return source.slice(start, index - 1);
      }
      fail("Close the LaTeX brace.");
    };
    const readOptionalGroup = () => {
      skip(); if (source[index] !== "[") return null;
      index++; const start = index; let depth = 1, escaped = false, inString = false;
      while (index < source.length) {
        const char = source[index++];
        if (inString) {
          if (escaped) { escaped = false; continue; }
          if (char === "\\") { escaped = true; continue; }
          if (char === '"') inString = false;
          continue;
        }
        if (escaped) { escaped = false; continue; }
        if (char === "\\") { escaped = true; continue; }
        if (char === '"') { inString = true; continue; }
        if (char === "[") depth++;
        else if (char === "]" && --depth === 0) return source.slice(start, index - 1);
      }
      fail("Close the optional root degree.");
    };
    const readPropertyReference = () => {
      skip();
      if (source[index] !== "{") fail("Property names must be quoted strings inside braces.");
      index++;
      skip();
      if (source[index] !== '"') fail("Property names must be quoted strings inside braces.");
      const start = index;
      index++;
      let escaped = false;
      while (index < source.length) {
        const character = source[index++];
        if (escaped) { escaped = false; continue; }
        if (character === "\\") { escaped = true; continue; }
        if (character === '"') break;
      }
      const encodedName = source.slice(start, index);
      skip();
      if (source[index] !== "}") fail("Close the property name brace.");
      index++;
      let name;
      try { name = JSON.parse(encodedName); }
      catch (_) { fail("Property names must be valid quoted strings."); }
      if (typeof name !== "string") fail("Property names must be quoted strings inside braces.");
      let ordinal = null;
      const selector = /^#(\d+)/.exec(source.slice(index));
      if (selector) {
        index += selector[0].length;
        ordinal = Number(selector[1]);
        if (!Number.isSafeInteger(ordinal) || ordinal < 1) fail("Property selectors start at #1.");
      }
      const matches = (properties || [])
        .filter(item => isSelectableProperty(item) && item.name === name)
        .sort((left, right) => left.key < right.key ? -1 : left.key > right.key ? 1 : 0);
      if (!matches.length) fail("This property is unavailable in the current view.");
      if (ordinal === null && matches.length > 1) fail("This property name is ambiguous; choose its numbered reference from the property picker.");
      const property = matches[ordinal === null ? 0 : ordinal - 1];
      if (!property) fail("This numbered property reference is unavailable in the current view.");
      skip();
      if (source[index] !== "{") return token("property", { property: property.key });
      let operation;
      try { operation = JSON.parse(readGroup()); }
      catch (_) { fail("Aggregation must be a quoted operation name."); }
      if (typeof operation !== "string") fail("Aggregation must be a quoted operation name.");
      const normalizedOperation = operation.trim().toLowerCase();
      if (!Object.hasOwn(CANONICAL_OPERATION, normalizedOperation)) fail("Choose avg, total, sum, median, min, or max for a property.");
      const canonical = CANONICAL_OPERATION[normalizedOperation];
      skip();
      let calculation;
      if (source[index] === "{") calculation = parseLatexInternal(readGroup(), properties, true, nesting + 1);
      return token("aggregate", calculation ? { operation: canonical, property: property.key, calculation } : { operation: canonical, property: property.key });
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
      if (depth + nesting > 48) fail("This calculation is nested too deeply.");
      skip(); let output = [];
      if (source[index] === "+" || source[index] === "-") {
        output.push(token("operator", { value: source[index++] }));
        output.push(...expression(20, depth + 1));
      } else if (source[index] === "(" || source.startsWith("\\left(", index)) {
        const close = matchParen("(", ")");
        output.push(token("open"), ...expression(0, depth + 1)); close(); output.push(token("close"));
      } else if (source[index] === "{") {
        output.push(token("open"), ...parseLatexInternal(readGroup(), properties, allowPerCard, nesting + 1), token("close"));
      } else if (source[index] === "\\") {
        const command = readCommand();
        if (command === "sqrt") {
          const optional = readOptionalGroup();
          const body = readGroup();
          if (optional === null) output.push(token("function", { value: "sqrt" }), token("open"), ...parseLatexInternal(body, properties, allowPerCard, nesting + 1), token("close"));
          else output.push(token("function", { value: "root" }), token("open"), ...parseLatexInternal(body, properties, allowPerCard, nesting + 1), token("comma"), ...parseLatexInternal(optional, properties, allowPerCard, nesting + 1), token("close"));
        } else if (command === "frac") {
          const numerator = parseLatexInternal(readGroup(), properties, allowPerCard, nesting + 1), denominator = parseLatexInternal(readGroup(), properties, allowPerCard, nesting + 1);
          output.push(token("open"), ...numerator, token("close"), token("operator", { value: "/" }), token("open"), ...denominator, token("close"));
        } else if (command === "property") {
          const ref = readPropertyReference();
          if (ref.kind === "property" && !allowPerCard) fail("A per-card property requires an aggregation calculation.");
          output.push(ref);
        } else if (command === "count") output.push(token("count"));
        else fail("Unsupported LaTeX command.");
      } else {
        const number = /^(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?/.exec(source.slice(index));
        if (number) { index += number[0].length; output.push(token("number", { value: number[0] })); }
        else fail("Enter a number, aggregate, count, or supported LaTeX function.");
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
    parseTokens(tokens, { allowPerCard });
    return tokens;
  }

  UI.CalculatorMath = { parseTokens, parseLatex, toLatex, toMathML, toMathMLBody, groupedFraction, propertyReference };
})(window);
