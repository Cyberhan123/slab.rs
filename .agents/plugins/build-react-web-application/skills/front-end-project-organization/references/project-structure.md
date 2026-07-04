# 🗄️ Project Structure

Most of the code lives in the `src` folder and looks something like this:

```sh
src
│
├── routes/
│   ├── modules/           
│   │   ├── dashboard.ts
│   │   ├── user.ts
│   │   └── setting.ts
│   ├── config.tsx         
│   ├── Guard.tsx         
│   └── index.tsx
│      
├── layouts/           
│   ├── MainLayout.tsx
│   └── XXXLayout.tsx
│
├── assets/            # assets folder can contain all the static files such as images, fonts, etc.
│
├── components/        # shared components used across the entire application
│
├── config/            # global configurations, exported env variables etc.
│
├── pages/          # feature based modules
│
├── hooks/             # shared hooks used across the entire application
│
├── lib/               # reusable libraries preconfigured for the application
│
├── stores/            # global state stores
│
├── tests/         # test utilities and mocks
│
├── types/             # shared types used across the application
│
└── utils/            # shared utility functions
```

For easy scalability and maintenance, organize most of the code within the pages folder. Each page folder should contain code specific to that page feature, keeping things neatly separated. This approach helps prevent mixing feature-related code with shared components, making it simpler to manage and maintain the codebase compared to having many files in a flat folder structure. By adopting this method, you can enhance collaboration, readability, and scalability in the application's architecture.

A page feature could have the following structure:

```sh
src/pages/awesome-page
│
├── components  # components scoped to a specific feature
│
├── hooks       # hooks scoped to a specific feature
│
├── stores      # state stores for a specific feature
│
├── types       # typescript types used within the feature
│
└── utils       # utility functions for a specific feature
```

NOTE: You don't need all of these folders for every feature. Only include the ones that are necessary for the feature.

In some cases it might be more practical to keep all API calls outside of the page features folders in a dedicated `api` folder where all API calls are defined. This can be useful if you have a lot of shared API calls between page features.

In the past, it was recommended to use barrel files to export all the files from a feature. However, it can cause issues for Vite to do tree shaking and can lead to performance issues. Therefore, it is recommended to import the files directly.

It might not be a good idea to import across the page features. Instead, compose different page features at the application level. This way, you can ensure that each feature is independent which makes the codebase less convoluted.

```

You might also want to enforce unidirectional codebase architecture. This means that the code should flow in one direction, from shared parts of the code to the application (shared ->page features -> app). This is a good practice to follow as it makes the codebase more predictable and easier to understand.

As you can see, the shared parts can be used by any part of the codebase, but the page can only import from shared parts and the app(src dir) can import from page and shared parts.


By following these practices, you can ensure that your codebase is well-organized, scalable, and maintainable. This will help you and your team to work more efficiently and effectively on the project.
This approach can also make it easier to apply similar architecture to apps built with Next.js, Remix or React Native.