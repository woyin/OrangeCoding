/**
 * WebSearchTool — searches the web and returns structured results.
 *
 * Uses DuckDuckGo HTML endpoint (no API key required) as primary,
 * with a fallback to Wikipedia for entity lookups.
 * Replaces the StubTool placeholder.
 */

import type { Tool, ToolMetadata } from "./tool.js";
import { ToolError } from "./tool.js";
import { readOnlyMetadata } from "./tool.js";

// ---------------------------------------------------------------------------
// WebSearchTool
// ---------------------------------------------------------------------------

interface WebSearchArgs {
  query: string;
  max_results?: number;
}

const MAX_RESULTS = 10;
const REQUEST_TIMEOUT = 15_000;

export class WebSearchTool implements Tool {
  private readonly _params: Record<string, unknown>;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        query: { type: "string", description: "Search query" },
        max_results: { type: "integer", description: "Maximum results (default 5, max 10)" },
      },
      required: ["query"],
    };
  }

  name(): string { return "web_search"; }
  description(): string {
    return "Search the web for information. Returns titles, URLs, and snippets. " +
      "Use for looking up documentation, APIs, error messages, or any factual query.";
  }
  parameters(): Record<string, unknown> { return this._params; }
  metadata(): ToolMetadata { return readOnlyMetadata(); }

  async execute(_ctx: unknown, input: unknown): Promise<string> {
    const args = input as WebSearchArgs;
    if (!args.query || !args.query.trim()) {
      throw new ToolError("invalid_params", "query is required");
    }

    const maxResults = Math.min(args.max_results || 5, MAX_RESULTS);
    const query = args.query.trim();

    // Try DuckDuckGo HTML endpoint (no API key, no JS rendering needed)
    try {
      const results = await this.searchDuckDuckGo(query, maxResults);
      if (results.length > 0) {
        return this.formatResults(results, query);
      }
    } catch {
      // Fall through to Wikipedia
    }

    // Fallback: Wikipedia search
    try {
      const results = await this.searchWikipedia(query, maxResults);
      if (results.length > 0) {
        return this.formatResults(results, query);
      }
    } catch {
      // Fall through
    }

    return `No search results found for: "${query}"`;
  }

  private async searchDuckDuckGo(query: string, maxResults: number): Promise<SearchResult[]> {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT);

    try {
      const url = `https://html.duckduckgo.com/html/?q=${encodeURIComponent(query)}`;
      const resp = await fetch(url, {
        signal: controller.signal,
        headers: {
          "User-Agent": "Mozilla/5.0 (compatible; OrangeCoding/1.0)",
          "Accept": "text/html",
        },
      });

      if (!resp.ok) {
        throw new Error(`HTTP ${resp.status}`);
      }

      const html = await resp.text();
      return this.parseDuckDuckGoHtml(html, maxResults);
    } finally {
      clearTimeout(timeout);
    }
  }

  private parseDuckDuckGoHtml(html: string, maxResults: number): SearchResult[] {
    const results: SearchResult[] = [];

    // DuckDuckGo HTML version uses .result class with .result__a (title) and .result__snippet
    // Simple regex-based extraction (no HTML parser dependency)
    const resultBlocks = html.split(/class="result /);

    for (let i = 1; i < resultBlocks.length && results.length < maxResults; i++) {
      const block = resultBlocks[i]!;

      // Extract title and URL from result__a link
      const titleMatch = block.match(/class="result__a"[^>]*href="([^"]+)"[^>]*>([\s\S]*?)<\/a>/);
      if (!titleMatch) continue;

      let url = titleMatch[1]!;
      // DDG wraps URLs in a redirect
      const uddgMatch = url.match(/[?&]uddg=([^&]+)/);
      if (uddgMatch) {
        try {
          url = decodeURIComponent(uddgMatch[1]!);
        } catch {
          // keep original
        }
      }

      const title = this.stripHtml(titleMatch[2]!);

      // Extract snippet
      const snippetMatch = block.match(/class="result__snippet"[^>]*>([\s\S]*?)<\/(?:a|span|div)/);
      const snippet = snippetMatch ? this.stripHtml(snippetMatch[1]!) : "";

      if (title && url) {
        results.push({ title, url, snippet });
      }
    }

    return results;
  }

  private async searchWikipedia(query: string, maxResults: number): Promise<SearchResult[]> {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT);

    try {
      const url = `https://en.wikipedia.org/w/api.php?action=opensearch&search=${encodeURIComponent(query)}&limit=${maxResults}&format=json`;
      const resp = await fetch(url, {
        signal: controller.signal,
        headers: { "User-Agent": "OrangeCoding/1.0 (coding agent)" },
      });

      if (!resp.ok) {
        throw new Error(`HTTP ${resp.status}`);
      }

      const data = await resp.json() as [string, string[], string[], string[]];
      const titles = data[1] || [];
      const descriptions = data[2] || [];
      const urls = data[3] || [];

      const results: SearchResult[] = [];
      for (let i = 0; i < titles.length && i < maxResults; i++) {
        results.push({
          title: titles[i]!,
          url: urls[i] || `https://en.wikipedia.org/wiki/${encodeURIComponent(titles[i]!)}`,
          snippet: descriptions[i] || "",
        });
      }

      return results;
    } finally {
      clearTimeout(timeout);
    }
  }

  private formatResults(results: SearchResult[], query: string): string {
    const lines: string[] = [];
    lines.push(`Search results for: "${query}"\n`);

    for (let i = 0; i < results.length; i++) {
      const r = results[i]!;
      lines.push(`${i + 1}. ${r.title}`);
      lines.push(`   URL: ${r.url}`);
      if (r.snippet) {
        lines.push(`   ${r.snippet}`);
      }
      lines.push("");
    }

    return lines.join("\n");
  }

  private stripHtml(html: string): string {
    return html
      .replace(/<[^>]*>/g, "")
      .replace(/&amp;/g, "&")
      .replace(/&lt;/g, "<")
      .replace(/&gt;/g, ">")
      .replace(/&quot;/g, '"')
      .replace(/&#0?39;/g, "'")
      .replace(/&nbsp;/g, " ")
      .replace(/\s+/g, " ")
      .trim();
  }
}

interface SearchResult {
  title: string;
  url: string;
  snippet: string;
}
