export const useStyle = () => {
  return {
    layout: "w-full h-full flex bg-background overflow-hidden",
    side: "bg-sidebar w-72 h-full flex flex-col p-3 box-border border-r border-border",
    logo: "flex items-center justify-start px-6 gap-2 my-6",
    logoText: "font-bold text-foreground text-base",
    conversations: "overflow-y-auto mt-3 p-0 flex-1",
    sideFooter: "border-t border-border h-10 flex items-center justify-between px-4",
    chat: "h-full flex-1 flex flex-col py-6 px-6 gap-4",
    startPage: "flex w-full max-w-4xl flex-col items-center h-full",
    agentName: "mt-[25%] text-2xl mb-9 font-semibold text-foreground",
    chatList: "flex w-full h-full flex-col justify-between",
    messageList: "w-full max-w-4xl flex-1 overflow-y-auto",
    inputArea: "w-full max-w-4xl",

    bubbleUpdating: "bg-gradient-to-r from-accent via-purple-500 to-primary bg-[length:100%_2px] bg-no-repeat bg-bottom",
  };
};