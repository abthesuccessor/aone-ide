import { AppShell } from "./app/AppShell";
import { useAppController } from "./app/useAppController";
import { useDeveloperWorkbench } from "./app/useDeveloperWorkbench";

export default function App() {
  const core = useAppController();
  const workbench = useDeveloperWorkbench(core);
  return <AppShell controller={{ ...core, ...workbench }} />;
}
