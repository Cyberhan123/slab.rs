function About() {
  return (
    <main className="max-w-4xl mx-auto p-8 bg-background min-h-screen">
      <div className="bg-white rounded-xl shadow-lg p-8">
        <h1 className="text-3xl font-bold text-center text-text mb-8">About Slab App</h1>
        <div className="space-y-6">
          <div>
            <h2 className="text-2xl font-bold text-text mb-4">About Page</h2>
            <p className="text-muted-foreground">This is the about page of the Slab App.</p>
          </div>
          <div className="p-4 bg-primary/5 rounded-lg border border-primary/10">
            <h3 className="text-lg font-semibold text-primary mb-2">Features</h3>
            <ul className="list-disc list-inside space-y-2 text-muted-foreground">
              <li>Native desktop performance with Tauri</li>
              <li>Modern React frontend</li>
              <li>Eye-friendly cyan color scheme</li>
              <li>Responsive design</li>
              <li>Secure Rust backend</li>
            </ul>
          </div>
        </div>
      </div>
    </main>
  );
}

export default About;