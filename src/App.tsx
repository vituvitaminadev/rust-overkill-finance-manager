import { UpdatePrompt } from "@/components/UpdatePrompt";
import { errorMessage } from "@/lib/api";
import { useStatus } from "@/lib/session";
import { AuthScreen } from "@/screens/AuthScreen";
import { HouseholdsScreen } from "@/screens/HouseholdsScreen";
import { Workspace } from "@/screens/Workspace";

function App() {
  return (
    <>
      <Screen />
      <UpdatePrompt />
    </>
  );
}

function Screen() {
  const status = useStatus();

  if (status.error) {
    return (
      <p role="alert" className="p-8 text-center text-destructive">
        {errorMessage(status.error)}
      </p>
    );
  }
  if (!status.data) return null;

  const { current, username, hasHouseholds } = status.data;
  if (current) {
    // A chave descarta filtros e abas de uma residência ao abrir outra.
    return (
      <Workspace
        key={current.household.id}
        household={current.household}
        me={current.member}
      />
    );
  }
  if (username) return <HouseholdsScreen status={status.data} />;
  return <AuthScreen hasHouseholds={hasHouseholds} />;
}

export default App;
