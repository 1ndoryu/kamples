import TopMenu from "@/components/TopMenu";
import SidebarMenu from "@/components/SidebarMenu";
import SamplesFeed from "@/components/SamplesFeed";

export default function Home() {
  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100vh" }}>
      <TopMenu />
      <div style={{ display: "flex", flex: 1 }}>
        <SidebarMenu />
        <main style={{ flex: 1, padding: "16px", overflowY: "auto" }}>
          <SamplesFeed />
        </main>
      </div>
    </div>
  );
}
