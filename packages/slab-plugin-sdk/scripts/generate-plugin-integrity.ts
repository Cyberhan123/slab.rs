import { updatePluginManifestIntegrity } from "../src/integrity";

const pluginDir = process.argv[2] ?? process.cwd();
const filesSha256 = await updatePluginManifestIntegrity(pluginDir);

console.log(
  `Updated plugin integrity for ${Object.keys(filesSha256).length} files in ${pluginDir}`,
);
