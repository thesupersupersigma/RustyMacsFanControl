// Creates functions at code addresses that are only referenced through data pointers
// (vtables, callback tables), which auto-analysis leaves undefined. Run as -preScript.
import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.mem.MemoryBlock;

public class RecoverPointerFuncs extends GhidraScript {
    @Override
    public void run() throws Exception {
        Memory mem = currentProgram.getMemory();
        int created = 0, candidates = 0;
        for (MemoryBlock block : mem.getBlocks()) {
            if (!block.isInitialized() || block.isExecute()) continue;
            Address a = block.getStart();
            long n = block.getSize() / 4;
            for (long i = 0; i < n && !monitor.isCancelled(); i++, a = a.add(4)) {
                long v = mem.getInt(a) & 0xffffffffL;
                Address target = a.getNewAddress(v);
                MemoryBlock tb = mem.getBlock(target);
                if (tb == null || !tb.isExecute()) continue;
                if (getFunctionContaining(target) != null) continue;
                Instruction ins = getInstructionAt(target);
                if (ins == null) {
                    // only start disassembly on undefined bytes, never inside an instruction
                    if (getInstructionContaining(target) != null || getDataAt(target) != null
                            && getDataAt(target).isDefined()) continue;
                    new DisassembleCommand(target, null, true).applyTo(currentProgram, monitor);
                    ins = getInstructionAt(target);
                    if (ins == null) continue;
                }
                candidates++;
                if (createFunction(target, null) != null) created++;
            }
        }
        println("RecoverPointerFuncs: " + created + " functions created from " + candidates + " pointer targets");
    }
}
