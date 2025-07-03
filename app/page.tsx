import TopMenu from "../components/TopMenu";
import SidebarMenu from "../components/SidebarMenu";
import SamplesFeed from "../components/SamplesFeed";

export default function Home() {
  return (
    <div style={{ display: "flex", flexDirection: "column", width: "100%", height: "100%"}}>
      <TopMenu />
      <div style={{ display: "flex", flex: 1 }}>
        <SidebarMenu />
        <main style={{ flex: 1, padding: "74px", overflowY: "auto", paddingRight: "20px" }}>
          <SamplesFeed />
        </main>
      </div>
    </div>
  );
}
