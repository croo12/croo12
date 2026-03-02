import { useState } from "react";
import { colors, layout, spacing } from "@/shared/theme";
import { Body, SubTitle, Title, useTypewriter } from "@/shared/ui";

const OVERVIEW_TEXT =
	"WebAssembly 기술을 테스트하기 위한 실험 페이지입니다. 네이티브 코드를 .wasm으로 빌드하여 브라우저에서 실행하고, Canvas 기반으로 결과를 렌더링합니다.";

const TECH_STACK = ["WebAssembly (Rust)", "React", "TypeScript"];

/*
 * Step 0:     h1  "About"
 * Step 1:     h2  "Project Overview"
 * Step 2:     p   overview text
 * Step 3:     h2  "Tech Stack"
 * Step 4–6:   li  tech stack items (last — cursor stays)
 */
const TECH_STACK_START = 4;

interface TypedLineProps {
	text: string;
	speed?: number;
	isActive: boolean;
	showCursor: boolean;
	onComplete: () => void;
}

const TypedLine: React.FC<TypedLineProps> = ({
	text,
	speed,
	isActive,
	showCursor,
	onComplete,
}) => {
	const { displayedText } = useTypewriter({
		text,
		speed,
		isActive,
		onComplete,
	});

	return (
		<>
			{displayedText}
			{showCursor && <span className="cursor" aria-hidden="true" />}
		</>
	);
};

export const AboutPage: React.FC = () => {
	const [step, setStep] = useState(0);

	const next = (): void => setStep((s) => s + 1);

	return (
		<div
			style={{
				maxWidth: layout.maxWidthNarrow,
				margin: "0 auto",
				padding: `${spacing["2xl"]} ${spacing.md}`,
			}}
		>
			<Title>
				<TypedLine
					text="About"
					speed={80}
					isActive={step >= 0}
					showCursor={step === 0}
					onComplete={next}
				/>
			</Title>

			{step >= 1 && (
				<section style={{ marginBottom: spacing.xl }}>
					<SubTitle>
						<TypedLine
							text="Project Overview"
							speed={40}
							isActive={step >= 1}
							showCursor={step === 1}
							onComplete={next}
						/>
					</SubTitle>
					{step >= 2 && (
						<Body>
							<TypedLine
								text={OVERVIEW_TEXT}
								speed={10}
								isActive={step >= 2}
								showCursor={step === 2}
								onComplete={next}
							/>
						</Body>
					)}
				</section>
			)}

			{step >= 3 && (
				<section style={{ marginBottom: spacing.xl }}>
					<SubTitle>
						<TypedLine
							text="Tech Stack"
							speed={40}
							isActive={step >= 3}
							showCursor={step === 3}
							onComplete={next}
						/>
					</SubTitle>
					{step >= TECH_STACK_START && (
						<ul style={{ color: colors.textMuted, lineHeight: 2 }}>
							{TECH_STACK.map((item, i) => {
								const s = TECH_STACK_START + i;
								return (
									step >= s && (
										<li key={item}>
											<TypedLine
												text={item}
												speed={20}
												isActive={step >= s}
												showCursor={step === s}
												onComplete={next}
											/>
										</li>
									)
								);
							})}
						</ul>
					)}
				</section>
			)}

		</div>
	);
};
