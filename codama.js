export default {
  idl: 'target/idl/agent_vault.json',
  scripts: {
    js: {
      from: '@codama/renderers-js',
      args: [
        'clients/js',
        {
          formatCode: true,
          deleteFolderBeforeRendering: true,
        },
      ],
    },
  },
};
