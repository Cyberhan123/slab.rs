import { useState } from "react";
import reactLogo from "./assets/react.svg";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "./store/useAppStore";

function App() {
  const [name, setName] = useState("");
  const { isLoading, greetMessage, setIsLoading, setGreetMessage } = useAppStore();

  const greet = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    
    setIsLoading(true);
    try {
      const result = await invoke("greet", { name });
      setGreetMessage(result as string);
    } catch (error) {
      console.error("Error calling greet command:", error);
      setGreetMessage("An error occurred. Please try again.");
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <main className="max-w-4xl mx-auto p-6 bg-gray-50 min-h-screen">
      <h1 className="text-3xl font-bold text-center text-gray-800 mb-8">Welcome to Slab App</h1>

      <div className="flex items-center justify-center gap-6 mb-6">
        <a href="https://vite.dev" target="_blank" rel="noopener noreferrer">
          <img src="/vite.svg" className="h-16" alt="Vite logo" />
        </a>
        <a href="https://tauri.app" target="_blank" rel="noopener noreferrer">
          <img src="/tauri.svg" className="h-16" alt="Tauri logo" />
        </a>
        <a href="https://react.dev" target="_blank" rel="noopener noreferrer">
          <img src={reactLogo} className="h-16" alt="React logo" />
        </a>
      </div>
      <p className="text-center text-gray-600 mb-8">Click on the logos to learn more about the technologies used.</p>

      <form className="flex flex-col items-center gap-4 max-w-md mx-auto" onSubmit={greet}>
        <input
          id="greet-input"
          value={name}
          onChange={(e) => setName(e.currentTarget.value)}
          placeholder="Enter your name..."
          disabled={isLoading}
          className="w-full px-4 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
        />
        <button 
          type="submit" 
          disabled={isLoading}
          className="px-6 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 transition-colors disabled:bg-gray-400 disabled:cursor-not-allowed"
        >
          {isLoading ? "Loading..." : "Greet"}
        </button>
      </form>
      {greetMessage && <p className="text-center text-gray-800 mt-6 p-4 bg-white rounded-md shadow-sm">{greetMessage}</p>}
    </main>
  );
}

export default App;
