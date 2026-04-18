import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"
import {
  Avatar,
  AvatarFallback,
  AvatarBadge,
  AvatarGroup,
  AvatarGroupCount
} from "@/avatar"
import { renderComponentScene } from "../test-utils"

function AvatarGallery() {
  return (
    <div data-testid="avatar-gallery" className="flex flex-wrap items-center gap-4 rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm">
      <Avatar size="sm" data-testid="avatar">
        <AvatarFallback>SM</AvatarFallback>
      </Avatar>
      <Avatar size="default">
        <AvatarFallback>DF</AvatarFallback>
      </Avatar>
      <Avatar size="lg">
        <AvatarFallback>LG</AvatarFallback>
      </Avatar>
      <Avatar>
        <AvatarFallback>JD</AvatarFallback>
        <AvatarBadge />
      </Avatar>
      <AvatarGroup size="default">
        <Avatar>
          <AvatarFallback>A</AvatarFallback>
        </Avatar>
        <Avatar>
          <AvatarFallback>B</AvatarFallback>
        </Avatar>
        <Avatar>
          <AvatarFallback>C</AvatarFallback>
        </Avatar>
        <AvatarGroupCount>+3</AvatarGroupCount>
      </AvatarGroup>
    </div>
  )
}

describe("Avatar browser coverage", () => {
  it("matches the shared avatar gallery screenshot", async () => {
    await renderComponentScene(<AvatarGallery />)
    const avatar = page.getByTestId("avatar")
    await expect.element(avatar).toBeVisible()
    await expect(page.getByTestId("avatar-gallery")).toMatchScreenshot("avatar-gallery.png")
  })

  it("keeps interactive avatar structure covered", async () => {
    await renderComponentScene(
      <Avatar size="lg" data-testid="test-avatar">
        <AvatarFallback data-testid="avatar-fallback">TS</AvatarFallback>
      </Avatar>
    )
    const avatar = page.getByTestId("test-avatar")
    await expect.element(avatar).toBeVisible()
    await expect.element(avatar).toHaveAttribute("data-size", "lg")

    const fallback = page.getByTestId("avatar-fallback")
    await expect.element(fallback).toBeVisible()
    await expect.element(fallback).toHaveTextContent("TS")
  })
})
