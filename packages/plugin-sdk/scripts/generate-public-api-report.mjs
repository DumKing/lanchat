import ts from "typescript";
import { readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";

const sdkRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function sortedRecord(value) {
  return Object.fromEntries(Object.entries(value).sort(([left], [right]) => left < right ? -1 : left > right ? 1 : 0));
}

export async function createPublicApiReport() {
  const declarationPath = resolve(sdkRoot, "dist/index.d.ts");
  const program = ts.createProgram([declarationPath], {
    target: ts.ScriptTarget.ES2020,
    module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    skipLibCheck: true,
  });
  const diagnostics = ts.getPreEmitDiagnostics(program);
  if (diagnostics.length > 0) {
    throw new Error(ts.formatDiagnosticsWithColorAndContext(diagnostics, {
      getCanonicalFileName: (fileName) => fileName,
      getCurrentDirectory: () => sdkRoot,
      getNewLine: () => "\n",
    }));
  }

  const source = program.getSourceFile(declarationPath);
  const moduleSymbol = source && program.getTypeChecker().getSymbolAtLocation(source);
  if (!moduleSymbol) throw new Error(`Cannot resolve declarations from ${declarationPath}`);
  const checker = program.getTypeChecker();
  const exportedTypes = checker.getExportsOfModule(moduleSymbol)
    .filter((symbol) => {
      const target = (symbol.flags & ts.SymbolFlags.Alias) !== 0 ? checker.getAliasedSymbol(symbol) : symbol;
      return (target.flags & ts.SymbolFlags.Type) !== 0;
    })
    .map((symbol) => symbol.getName())
    .sort();

  const runtime = await import(`${pathToFileURL(resolve(sdkRoot, "dist/index.js")).href}?report=${Date.now()}`);
  const client = runtime.createLanChatPluginClient({
    request: async () => undefined,
    subscribe: () => () => undefined,
  });
  const clientMethods = {};
  for (const namespace of Object.keys(client).filter((key) => key !== "apiVersion").sort()) {
    clientMethods[namespace] = Object.keys(client[namespace]).sort();
  }

  return {
    schemaVersion: 1,
    exportedTypes,
    runtimeExports: Object.keys(runtime).sort(),
    runtimeConstants: {
      PLUGIN_EVENTS: sortedRecord(runtime.PLUGIN_EVENTS),
      PLUGIN_HASH_CONTRACT: sortedRecord(runtime.PLUGIN_HASH_CONTRACT),
    },
    clientMethods,
  };
}

export function serializePublicApiReport(report) {
  return `${JSON.stringify(report, null, 2)}\n`;
}

async function main() {
  const reportText = serializePublicApiReport(await createPublicApiReport());
  const args = process.argv.slice(2);
  const checkIndex = args.indexOf("--check");
  const outputIndex = args.indexOf("--output");
  if (checkIndex >= 0) {
    const target = args[checkIndex + 1];
    if (!target) throw new Error("--check requires a report path");
    const expected = await readFile(resolve(target), "utf8");
    if (expected !== reportText) {
      throw new Error(`Public API report is stale: ${resolve(target)}. Regenerate it explicitly.`);
    }
    return;
  }
  if (outputIndex >= 0) {
    const target = args[outputIndex + 1];
    if (!target) throw new Error("--output requires a report path");
    await writeFile(resolve(target), reportText, "utf8");
    return;
  }
  process.stdout.write(reportText);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  });
}
