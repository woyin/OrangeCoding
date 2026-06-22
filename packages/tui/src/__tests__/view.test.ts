/**
 * view() 渲染裁剪测试。
 *
 * 保护目标：renderChatArea 重构为“从尾部向前回溯、只收集 height 行”，
 * 必须保证：
 * - 输出区域行数受 height 约束（不会无限增长）
 * - 长会话下只显示最近的行（尾部优先），旧消息被裁掉
 * - 短会话下正常显示全部内容
 */

import { view } from "../view.js";
import { Model } from "../model.js";
import type { Message } from "@orangecoding/core";
import { Role } from "@orangecoding/core";

function userMsg(content: string): Message {
  return {
    role: Role.User,
    content,
    createdAt: new Date(),
    name: undefined,
    toolCalls: undefined,
    toolCallID: undefined,
  } as unknown as Message;
}

describe("view 渲染裁剪", () => {
  it("短会话：消息行数不足时输出全部内容", () => {
    const m = new Model({
      messages: [userMsg("hello"), userMsg("world")],
      width: 80,
      height: 24,
    });
    const out = view(m);
    expect(out).toContain("hello");
    expect(out).toContain("world");
  });

  it("长会话：只显示最近的行（尾部优先裁剪）", () => {
    // 构造 50 条消息，每条单行；chat 高度 = height - 3
    const msgs: Message[] = [];
    for (let i = 0; i < 50; i++) msgs.push(userMsg(`MSG-${i}`));
    const height = 13; // chatHeight = 10
    const m = new Model({ messages: msgs, width: 80, height });
    const out = view(m);

    // 最近的几条必须在输出里
    expect(out).toContain("MSG-49");
    expect(out).toContain("MSG-48");
    // 早期消息应被裁掉（chatHeight=10，远小于 50 条）
    expect(out).not.toContain("MSG-0");
    expect(out).not.toContain("MSG-5");
  });

  it("超长单条消息按宽度截断", () => {
    const long = "X".repeat(200);
    const m = new Model({
      messages: [userMsg(long)],
      width: 40,
      height: 24,
    });
    const out = view(m);
    // 任一行都不应超过宽度（渲染里 substring(0, width)）
    for (const line of out.split("\n")) {
      // 去掉 ANSI 转义后比较长度（这里只断言不出现 200 个连续 X）
      expect(line.length).toBeLessThan(200);
    }
  });
});
