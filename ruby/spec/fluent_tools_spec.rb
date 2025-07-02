# frozen_string_literal: true

RSpec.describe FluentTools do
  it 'has a version number' do
    expect(FluentTools::VERSION).not_to be_nil
  end

  describe '.fluent_to_android' do
    it 'delegates to CommandExecutor' do
      command_executor = instance_double(FluentTools::CommandExecutor)
      allow(FluentTools::CommandExecutor).to receive(:new).and_return(command_executor)
      allow(command_executor).to receive(:fluent_to_android)

      described_class.fluent_to_android('input.ftl', 'output.xml')

      expect(command_executor).to have_received(:fluent_to_android).with('input.ftl', 'output.xml')
    end
  end

  describe '.android_to_fluent' do
    it 'delegates to CommandExecutor without original_fluent' do
      command_executor = instance_double(FluentTools::CommandExecutor)
      allow(FluentTools::CommandExecutor).to receive(:new).and_return(command_executor)
      allow(command_executor).to receive(:android_to_fluent)

      described_class.android_to_fluent('input.xml', 'output.ftl')

      expect(command_executor).to have_received(:android_to_fluent).with('input.xml', 'output.ftl', original_fluent: nil)
    end

    it 'delegates to CommandExecutor with original_fluent' do
      command_executor = instance_double(FluentTools::CommandExecutor)
      allow(FluentTools::CommandExecutor).to receive(:new).and_return(command_executor)
      allow(command_executor).to receive(:android_to_fluent)

      described_class.android_to_fluent('input.xml', 'output.ftl', original_fluent: 'original.ftl')

      expect(command_executor).to have_received(:android_to_fluent).with('input.xml', 'output.ftl', original_fluent: 'original.ftl')
    end
  end
end
