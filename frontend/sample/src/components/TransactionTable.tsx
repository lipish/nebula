import { Search, Calendar, Filter, MoreHorizontal } from "lucide-react";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";

const transactions = [
  { id: "TRX-8F2A9C1E", date: "06 Oct, 2025", time: "05:50 AM", amount: "$1,250.00", status: "Completed" },
  { id: "TRX-94B7E2F5", date: "07 Oct, 2025", time: "09:20 AM", amount: "$840.50", status: "Pending" },
  { id: "TRX-7E9C2F14", date: "08 Oct, 2025", time: "08:10 AM", amount: "$2,100.00", status: "Completed" },
  { id: "TRX-4B1C8F92", date: "12 Oct, 2025", time: "04:30 PM", amount: "$560.75", status: "Completed" },
];

const TransactionTable = () => {
  return (
    <div className="bg-card border border-border rounded-2xl p-6">
      {/* Filters */}
      <div className="flex items-center gap-3 mb-4">
        <div className="flex items-center gap-2 border border-border rounded-lg px-3 py-2 flex-1 max-w-[200px]">
          <Search className="h-4 w-4 text-muted-foreground" />
          <input
            type="text"
            placeholder="Search ..."
            className="bg-transparent text-sm outline-none w-full text-foreground placeholder:text-muted-foreground"
          />
        </div>
        <button className="flex items-center gap-2 border border-border rounded-lg px-3 py-2 text-sm text-muted-foreground hover:bg-accent transition-colors">
          <Calendar className="h-4 w-4" />
          Date
        </button>
        <button className="flex items-center gap-2 border border-border rounded-lg px-3 py-2 text-sm text-muted-foreground hover:bg-accent transition-colors">
          <Filter className="h-4 w-4" />
          All Status
        </button>
        <button className="flex items-center gap-2 border border-border rounded-lg px-3 py-2 text-sm text-muted-foreground hover:bg-accent transition-colors">
          <MoreHorizontal className="h-4 w-4" />
          More
        </button>
      </div>

      <Table>
        <TableHeader>
          <TableRow className="hover:bg-transparent">
            <TableHead className="w-10"><Checkbox /></TableHead>
            <TableHead className="font-medium">Trx ID</TableHead>
            <TableHead className="font-medium">Date</TableHead>
            <TableHead className="font-medium">Time</TableHead>
            <TableHead className="font-medium">Amount</TableHead>
            <TableHead className="font-medium">Status</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {transactions.map((tx) => (
            <TableRow key={tx.id}>
              <TableCell><Checkbox /></TableCell>
              <TableCell className="font-mono text-sm">{tx.id}</TableCell>
              <TableCell className="text-sm">{tx.date}</TableCell>
              <TableCell className="text-sm text-muted-foreground">{tx.time}</TableCell>
              <TableCell className="text-sm font-medium">{tx.amount}</TableCell>
              <TableCell>
                <span className={`text-xs font-medium px-2 py-1 rounded-full ${
                  tx.status === "Completed"
                    ? "bg-success/10 text-success"
                    : "bg-accent text-muted-foreground"
                }`}>
                  {tx.status}
                </span>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
};

export default TransactionTable;
