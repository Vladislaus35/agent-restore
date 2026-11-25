import { describe, expect, it } from "vitest";
import { formatBytes, formatPercent, shortenPath } from "../format";
import { translations } from "../../i18n/translations";

describe("format utilities", () => {
  it("formats byte sizes cleanly", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1024)).toBe("1.00 KB");
    expect(formatBytes(15 * 1024 * 1024)).toBe("15.0 MB");
  });

  it("formats percentages safely", () => {
    expect(formatPercent(0)).toBe("0%");
    expect(formatPercent(64.25)).toBe("64.3%");
    expect(formatPercent(140)).toBe("100.0%");
  });

  it("shortens long paths while preserving tail segments", () => {
    const short = shortenPath("C:/Users/dev/projects/awesome-app/src", 25);
    expect(short).toBe("…/awesome-app/src");
  });
});

describe("translations parity", () => {
  it("has identical top-level and nested keys for English and Turkish", () => {
    const flattenKeys = (obj: Record<string, unknown>, prefix = ""): string[] =>
      Object.entries(obj)
        .flatMap(([k, v]) => {
          const path = prefix ? `${prefix}.${k}` : k;
          if (v && typeof v === "object") {
            return flattenKeys(v as Record<string, unknown>, path);
          }
          return [path];
        })
        .sort();

    expect(flattenKeys(translations.en as unknown as Record<string, unknown>)).toEqual(
      flattenKeys(translations.tr as unknown as Record<string, unknown>),
    );
  });
});
