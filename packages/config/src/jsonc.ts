/**
 * jsonc.ts —— 极简的 JSONC（带注释的 JSON）预处理器。
 *
 * OrangeCoding 配置文件允许 行注释 与 块注释，便于文档化；
 * JSON.parse 无法处理，故 parseJSONC 先剥离注释（同时尊重字符串字面量），
 * 再返回可被 JSON.parse 解析的纯净 JSON。
 */
import { newConfigError } from "@orangecoding/core";

/**
 * ParseJSONC strips // line comments and /* *​/ block comments from a JSONC
 * input string while preserving comments inside quoted strings. It returns
 * clean JSON suitable for JSON.parse.
 */
export function parseJSONC(input: string): string {
  // 性能优化：原实现把每个非注释字符逐个 push 进数组，最后 join。
  // 对几 KB 的配置文件意味着上万次 push + 一次大字符串拼接。
  // 现在用“成段拷贝”：维护一个段起点 segStart，遇到注释才把
  // [segStart, i) 整段 slice 进结果，大幅减少 push 次数与分配。
  const parts: string[] = [];
  let segStart = 0;
  let inString = false;
  let escape = false;
  let i = 0;
  const n = input.length;

  const flush = (end: number) => {
    if (end > segStart) parts.push(input.slice(segStart, end));
  };

  while (i < n) {
    const ch = input[i]!;

    if (inString) {
      if (escape) {
        escape = false;
      } else if (ch === "\\") {
        escape = true;
      } else if (ch === '"') {
        inString = false;
      }
      i++;
      continue;
    }

    // 字符串外部
    if (ch === '"') {
      inString = true;
      i++;
    } else if (ch === "/" && i + 1 < n && input[i + 1] === "/") {
      // 行注释：先把已积累的段落盘，再跳到行尾
      flush(i);
      const end = input.indexOf("\n", i);
      if (end === -1) {
        i = n; // 剩余全是注释
      } else {
        i = end + 1; // 跳过 \n
      }
      segStart = i;
    } else if (ch === "/" && i + 1 < n && input[i + 1] === "*") {
      // 块注释：先把已积累的段落盘，再跳到 */
      flush(i);
      const end = input.indexOf("*/", i);
      if (end === -1) {
        throw newConfigError(
          `unterminated block comment starting at position ${i}`,
        );
      }
      i = end + 2; // 跳过 */
      segStart = i;
    } else {
      i++;
    }
  }

  if (inString) {
    throw newConfigError("unterminated string literal");
  }

  // 把最后一段拷进来
  flush(n);
  return parts.join("");
}
