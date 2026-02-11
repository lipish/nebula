import Sidebar from "@/components/Sidebar";
import ClusterSummary from "@/components/ClusterSummary";
import GpuUsageChart from "@/components/GpuUsageChart";
import EndpointTable from "@/components/EndpointTable";

const Index = () => {
  return (
    <div className="min-h-screen bg-background">
      <Sidebar />
      <main className="ml-64 p-8">
        <h2 className="text-2xl font-bold text-foreground mb-1">Overview</h2>

        <div className="mt-6 mb-2">
          <h3 className="text-xl font-bold text-foreground">Good Morning, Nero</h3>
          <p className="text-sm text-muted-foreground mt-1">Here's an overview of your cluster health and active models</p>
        </div>

        <div className="mt-5 space-y-5">
          <ClusterSummary />
          <GpuUsageChart />
          <EndpointTable />
        </div>
      </main>
    </div>
  );
};

export default Index;
