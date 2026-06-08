import type { SessionId } from "@orangecoding/core";

/**
 * SessionTree tracks parent-child relationships between sessions,
 * enabling session branching (forking).
 */
export class SessionTree {
  private parentOf = new Map<string, SessionId>();   // child -> parent
  private childrenOf = new Map<string, SessionId[]>(); // parent -> children

  /**
   * Fork registers a parent-child relationship between two sessions.
   */
  fork(parentID: SessionId, childID: SessionId): void {
    const childKey = childID.toString();
    const parentKey = parentID.toString();
    this.parentOf.set(childKey, parentID);
    const existing = this.childrenOf.get(parentKey) ?? [];
    existing.push(childID);
    this.childrenOf.set(parentKey, existing);
  }

  /**
   * GetChildren returns all child session IDs for the given parent.
   * Returns an empty array if the session has no children.
   */
  getChildren(id: SessionId): SessionId[] {
    return this.childrenOf.get(id.toString()) ?? [];
  }

  /**
   * GetParent returns the parent session ID if the given session has a parent.
   * Returns [sessionId, true] if found, or [undefined, false] if the session is a root.
   */
  getParent(id: SessionId): [SessionId | undefined, boolean] {
    const parent = this.parentOf.get(id.toString());
    if (parent !== undefined) {
      return [parent, true];
    }
    return [undefined, false];
  }
}
