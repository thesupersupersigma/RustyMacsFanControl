// Dumps decompiled C for every function, plus strings with their referencing functions.
// Headless usage: -postScript DumpAll.java <outDir>
import java.io.File;
import java.io.PrintWriter;
import java.util.LinkedHashSet;
import java.util.Set;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.data.StringDataInstance;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.DataIterator;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionIterator;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolIterator;

public class DumpAll extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        File outDir = new File(args[0], currentProgram.getName());
        outDir.mkdirs();

        try (PrintWriter w = new PrintWriter(new File(outDir, "strings.tsv"), "UTF-8")) {
            DataIterator it = currentProgram.getListing().getDefinedData(true);
            while (it.hasNext() && !monitor.isCancelled()) {
                Data d = it.next();
                StringDataInstance s = StringDataInstance.getStringDataInstance(d);
                if (s == null || s == StringDataInstance.NULL_INSTANCE) continue;
                String v = s.getStringValue();
                if (v == null || v.length() < 2) continue;
                Set<String> refs = new LinkedHashSet<>();
                ReferenceIterator ri = currentProgram.getReferenceManager().getReferencesTo(d.getAddress());
                while (ri.hasNext()) {
                    Reference r = ri.next();
                    Function f = getFunctionContaining(r.getFromAddress());
                    refs.add(f != null ? f.getEntryPoint().toString() : "@" + r.getFromAddress());
                }
                w.println(d.getAddress() + "\t" + String.join(",", refs) + "\t"
                        + v.replace("\\", "\\\\").replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t"));
            }
        }

        try (PrintWriter w = new PrintWriter(new File(outDir, "imports.tsv"), "UTF-8")) {
            SymbolIterator si = currentProgram.getSymbolTable().getExternalSymbols();
            while (si.hasNext()) {
                Symbol s = si.next();
                Set<String> refs = new LinkedHashSet<>();
                for (Reference r : s.getReferences()) {
                    Function f = getFunctionContaining(r.getFromAddress());
                    Address from = r.getFromAddress();
                    refs.add(f != null ? f.getEntryPoint().toString() : "@" + from);
                }
                w.println(s.getParentNamespace().getName() + "\t" + s.getName() + "\t" + String.join(",", refs));
            }
        }

        DecompInterface dec = new DecompInterface();
        dec.openProgram(currentProgram);
        int n = 0, failed = 0;
        try (PrintWriter w = new PrintWriter(new File(outDir, "decomp.c"), "UTF-8");
                PrintWriter idx = new PrintWriter(new File(outDir, "functions.tsv"), "UTF-8")) {
            FunctionIterator fi = currentProgram.getFunctionManager().getFunctions(true);
            while (fi.hasNext() && !monitor.isCancelled()) {
                Function f = fi.next();
                if (f.isExternal()) continue;
                Set<String> callees = new LinkedHashSet<>();
                for (Function c : f.getCalledFunctions(monitor)) {
                    callees.add(c.isExternal() || c.isThunk() ? c.getName() : c.getEntryPoint().toString());
                }
                idx.println(f.getEntryPoint() + "\t" + f.getName() + "\t" + f.getBody().getNumAddresses() + "\t"
                        + (f.isThunk() ? "thunk" : "") + "\t" + String.join(",", callees));
                if (f.isThunk()) continue;
                DecompileResults res = dec.decompileFunction(f, 90, monitor);
                w.println("//==== FUNC " + f.getEntryPoint() + " " + f.getName());
                if (res != null && res.decompileCompleted()) {
                    w.println(res.getDecompiledFunction().getC());
                } else {
                    failed++;
                    w.println("// DECOMPILE FAILED: " + (res == null ? "null" : res.getErrorMessage()));
                }
                if (++n % 1000 == 0) println("decompiled " + n);
            }
        }
        dec.dispose();
        println("DumpAll done: " + n + " functions, " + failed + " failed -> " + outDir);
    }
}
