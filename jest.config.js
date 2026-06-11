/** @type {import('jest').Config} */
export default {
  testEnvironment: "node",
  extensionsToTreatAsEsm: [".ts"],
  moduleNameMapper: {
    "^@orangecoding/(.+)$": "<rootDir>/packages/$1/src/index.ts",
    "^(\\.\\.?\\/.*)(\\.js)$": "$1",
  },
  transform: {
    "^.+\\.tsx?$": [
      "ts-jest",
      {
        useESM: true,
      },
    ],
  },
  testMatch: [
    "**/packages/*/src/**/*.test.ts",
    "**/packages/*/src/**/*.spec.ts",
    "**/packages/**/__tests__/**/*.test.ts",
  ],
  collectCoverageFrom: [
    "packages/*/src/**/*.ts",
    "!packages/*/src/**/*.d.ts",
    "!packages/**/__tests__/**",
  ],
  coverageDirectory: "coverage",
  coverageReporters: ["text", "lcov"],
  verbose: true,
};
